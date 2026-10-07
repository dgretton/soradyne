# The soradyne Cascade — Concepts, Docs, Implementation, Applications

> **Architecture revision, 2026-09-11:** read
> [shared-flow demo contracts](20260911_shared_flow_demo_contracts.md) first.
> It supersedes conflicting assumptions here about flow-owner authorship,
> cross-capsule disclosure, producer-local storage, unconditional discard,
> single-capsule-first sequencing, and assignment guarantees. This document
> remains historical rationale where it conflicts; no implementation is implied.


**Date**: 2026-09-06
**Author**: Claude (Fable 5.1, Claude Code), at Dana's request
**Audience**: agents and people doing soradyne work during the side-by-side period with
the coordinate-alignment project (see `20260906_fable_markov_blanket_discipline.md`),
and anyone cleaning up this repository.
**Status**: a plan with decisions in it. Currency **high** (grounded in the whole spec,
every design doc in `docs/`, the vault's values, the substrate contract on the other side
of the blanket, and the code as of today). Settled-ness **medium**: the concept verdicts
in §2 and the consolidated model in §3 are proposed as decisions; the application
sections in §6 are proposals; the retire list in §7 is a recommendation for Dana's
garden pass.

**Method in one sentence.** The substrate contract that the estimation application
needs (`estimation_flows.md`, and the spec passages added 2026-09-06) is treated as
the *first real load* soradyne has ever been asked to bear; every concept soradyne has
accumulated is held up against that load and against the vault's values, and anything
that neither is required by the load nor embodies a value is retired, until what remains
is one consistent system. That system is then used to re-derive the anchoring
applications: inventory, giantt, kmeep, and photos.

---

## 0. The two sources of truth

**The load.** The class of application described in `estimation_flows.md` §1: many
parties' observations, a designated party fusing them under a lease, everyone consuming
the estimate, scopes nesting by summary only, per-stream authorization along the
pipeline, several people contributing, the observations being the sensitive part. Its
requirements are already itemised (§3–§10 there). Everything below cites them as
**L-n** (L for load) by the section number of that document.

**The values.** From the vault (surveyed 2026-09-06; see the coordinate-alignment
mapping notes, kept outside this repo, §6.1): self-data is controlled, distributed,
granted and revoked by the person without a third party; your data lives with you and is
not a listening post; all data is derived, so what a receiver gets is a resampled
derivation appropriate to them; exposure is bounded and visible; tools touching self-data
are examinable; rim is not to become a surveillance actor from the other side. Cited
below as **V**.

A concept survives if it is required by **L**, or embodies **V** and is needed by an
anchoring application. A concept is *revised* if it survives but conflicts with another
survivor. A concept is *retired* if it is required by neither, or if it conflicts with a
survivor and loses.

---

## 1. Why a cascade, and why now

soradyne has accumulated two years of concepts from several directions: the original
collaborative-album system (the crate's own header still says "Collaborative Media Album
System"), dissolution storage, the BLE-first topology, the CRDT engine, the flows
abstraction, the TCP emulation, the granularity plan, and a privacy model that lives in
the vault and in correspondence but not in the spec. The 2026-09-06 assessments found
the granularity plan contradicting the spec, the auth model, and giantt's own semantics.
Every one of those conflicts arose because there was no application forcing a choice.

There is one now. The estimation application's needs are specific, privacy-heavy, and
multi-person, and they have been written down without reference to soradyne. That makes
them a usable source of truth: soradyne can be pruned against them without the pruning
being circular.

---

## 2. Concept inventory and verdicts

### 2.1 The flow model

| Concept | Verdict | Reason |
|---|---|---|
| Self-data flow: persistent, typed, UUID-identified bundle of streams with policies | **Keep** | L-2, L-10; the anchor of everything |
| Flow type as content-hashed design document | **Keep as intent, demote in practice** | Nothing needs the hash yet; string type names with a version suffix are the implementation. Keep the section; stop implying the hash exists |
| Flow configuration, bootstrap from UUID | **Keep** | L-10; apps hold UUIDs as opaque pointers |
| Type scope (device-specific vs gestalt) | **Keep** | Harmless, useful vocabulary |
| Stream: named, typed, with cardinality and interface | **Keep** | L-3 |
| Drip | **Revise** | Split into two kinds: *per-party keyed drip* (many writers, each owning keys) and *role-written drip* (one writer under lease, no CRDT needed). L-3.3 |
| Jet | **Keep, implement** | L-3.2. Declared, never built |
| Edit stream (per-party, in examples only) | **Promote to first-class: the log** | L-3.1. This is what `FullReplicaFlow`'s journals already are |
| Query-response stream | **Promote to first-class** | L-3.4. Image flow, lookup, asset fetch |
| Summary stream | **New** | L-3.5. A drip with a parent-only audience; the disclosure mechanism |
| Data geometry (query/data coordinates, transforms, sparsity) | **Keep as type-definition vocabulary, move to an appendix** | Nothing in L or any anchoring app uses it operationally; it is a good way to describe a type and a poor thing to put in the middle of the spec |
| Flow roles: source, sink, memorization, curator, aligner | **Keep** | L-4; the spec named aligner before it was needed |
| Fittings as non-first-class | **Keep** | L-2; the estimate is a fitting |

### 2.2 Policies

| Concept | Verdict | Reason |
|---|---|---|
| Error policies | **Revise: closed error set** | L-10. Six typed errors an application must handle; anything else is a bug |
| Delegation policies | **Revise: add eligibility and lease** | L-4. Capability, standing, authorization coverage, completeness; lease with role-gated writes |
| Discovery policies (proximity broadcast baseline; static, referral, registry, DHT) | **Keep** | Transport tiers doc already maps them |
| Memorization policies | **Revise: per stream, origin/replica, discard on leave** | L-6. The single most important privacy revision |
| Retirement and refresh | **Keep** | L; sessions are retirable flows |

### 2.3 Security and privacy

| Concept | Verdict | Reason |
|---|---|---|
| Data at rest tiers (plaintext-safe, encrypted, dissolution) | **Keep; rename to custody tiers** | L-6; V (bounded exposure) |
| Dissolution / crystallization (Shamir + Reed-Solomon) | **Keep** | V names it; it is the `dissolved` custody tier and the recording-corpus backend |
| Robustness (ephemeral, replicated, reallocating) | **Keep** | |
| Leakage policies | **Retire; replace with Privacy and Disclosure** | The section is about outsiders reading disk. The real threat model is people you shared with. The Ghost audit found "privacy" appears zero times in the spec |
| Authentication and authorization (four questions) | **Replace with Grants** | L-6, L-7, L-10: per-stream grants, introduction, revocation, the closed error set |
| Two-domain auth model (intra-capsule equal access; inter-capsule per-stream E2E) | **Keep** | It was right; the estimation load confirms the stream is the unit |
| Link model (app never sees capsules; embedded auth surface) | **Keep** | L-10; the sharing surface |
| Public cache / fidelity rule / unlinkability / revocation-you-can-feel (from correspondence, not in the spec) | **Adopt into the spec as Privacy and Disclosure** | V. These are the values applied to sharing, and every anchoring app needs them |

### 2.4 Flow boundaries and the mesh

| Concept | Verdict | Reason |
|---|---|---|
| Flow Boundary Principle (fitting necessity) | **Keep** | L-2 confirms it against the granularity plan |
| Guideline "atomic unit of data: an inventory's items" | **Retire the example** | It contradicts the inventory sharing requirement; the principle stands, the example was wrong |
| Participant scope, independent lifecycle, application-natural boundaries | **Keep** | |
| Flow mesh as non-protocol construct | **Revise** | L-7. References plus grants plus opaque references are protocol-level now |
| Nesting as disclosure | **New (added to spec 2026-09-06)** | L-7, V (all data is derived) |
| Collection flow as a blessed type (granularity plan) | **Revise down** | A collection is a per-party keyed drip of references plus an index. It needs no hint cache and no coherence slot. Keep the word |
| Item-is-the-flow / logical atomization | **Retire** | L-2. Continuity of identity is a *record* property (a stable key in a drip), not a flow property. See inventory in §6.1 |
| Graduated disclosure via cached hints on collections | **Retire; replaced by derivation** | V. The owner projects a derivation into a shared stream; the projection is authoritative for what it claims and is chosen per field by the owner. No cache-discipline problem, no write-back problem, and the title-caching contradiction the audit found disappears |
| Coherence groups | **Retire** | L confirms: one flow, many streams, atomic batches as records |
| Convergence ≠ validity; validation after materialisation | **Keep** | True for giantt and for estimation alike |

### 2.5 Device topology and transport

| Concept | Verdict | Reason |
|---|---|---|
| Cryptographic piece identity (Ed25519 + X25519) | **Keep** | L-10 identity; provenance |
| Piece, capsule (retired whole, add-only, multi-membership) | **Keep** | V; the fleet |
| Capsules as pre-flow infrastructure | **Keep** | |
| Ensemble (single-capsule live set) | **Keep** | |
| Flow ensemble | **New (design note added 2026-09-06)** | L-8 |
| Parure | **Keep as forward hardware concept, one paragraph** | Not needed by L; embodies V's body-network vision. Do not let it grow until hardware exists |
| Accessory | **Keep** | ESP32 shard holders are the `dissolved` tier's custodians |
| Routing as transport fabric; logical addressing; TTL envelopes | **Keep** | L-8; already built |
| BLE-first; TCP/mDNS/overlay as extensions; transport tiers | **Keep** | The tier doc already places overlay networks at 3a |
| "soradyne apps don't touch the radio" | **Keep** | |
| Clock reference | **New** | L-9 |

### 2.6 Application-facing surface

| Concept | Verdict | Reason |
|---|---|---|
| Generic `flow_open` / session / sync surface (auth model §6) | **Keep and build** | L-10; the FFI already has a generic registry, so this is a finishing job |
| App holds flow UUIDs as opaque pointers | **Keep** | |
| Schema knowledge stays in the app | **Keep** | The estimation load carries protobuf bytes through soradyne untouched |
| Persona / multichart many-to-many | **Retire the term** | Replaced by grants on streams and references |
| Dev-stub shared flow id | **Keep as a test convenience only** | |

### 2.7 Code-level concepts

| Concept | Verdict | Reason |
|---|---|---|
| `ConvergentDocument` (five ops, informed remove, horizon) | **Keep** | The per-party keyed drip's engine |
| `FullReplicaFlow` / `DripHostedFlow` alias (3.8 k lines) | **Split** | Its journals and horizon exchange become the *log* stream; its materialisation becomes the *keyed drip*; host assignment becomes *delegation*. See §5 |
| `DataChannel<T>` + `Diffable` | **Keep as the in-memory stream implementation** | It is a stream, as the spec says |
| Album module (`album/`: its own `Crdt`/`CrdtOp` traits, `LogCrdt`, `MediaAlbum`, `AlbumSyncManager`, renderer) | **Retire** | A second CRDT engine with its own sync protocol and content-hash addressing that conflicts with unlinkability. The photos app is rebuilt on the consolidated model (§6.4). Keep the renderer's resampling code if it is the cleanest way to compute image derivations |
| `BlockManager` / dissolution storage / erasure / galois / bcachefs backend | **Keep as the `dissolved` custody backend** | V; drop the album-specific `BlockManager` API surface |
| `types/` (heartrate, media, photos, robot_state, chat, files) | **Retire** | Demo-era types with no flow type behind them |
| `video/` (ffmpeg frame extraction) | **Retire from core** | A derivation function for a future video flow type; keep the code in an example or a plugin crate |
| `network/` (NetworkBridge, mDNS/LAN discovery) | **Fold into `ble/` transport backends** | Two discovery stacks exist; one should |
| `ffi/mod.rs` album FFI (`soradyne_get_albums`, `soradyne_upload_media`, ...) | **Retire** | Replaced by the generic surface |
| `ffi/pairing_bridge.rs` | **Keep, rename to session/identity surface** | Pairing, capsule listing, device id, sim accessories are the Link model's back end |
| `ffi/convergent_flow_ffi.rs` (`FLOW_REGISTRY`, `soradyne_flow_*`) | **Keep and extend** | Already generic; `soradyne_flow_connect_ensemble(capsule_id)` is the layer violation to remove |
| `examples/` (heartrate, robot joints, block storage, album sync, large video, web album server, renderer, sd card) and `bin/` (colmi bridge, sd card test) | **Retire** | Served their purpose. Rebuild the two or three that matter as examples of the consolidated model |
| `flow/examples/robot_joints.rs` | **Retire** | Same |
| `soradyne_cli` | **Keep and extend** | Capsule and flow management; gains stream-level commands |
| `soradyne_flutter` plugin (`FlowClient`, `SoradyneClient`) | **Keep; regenerate over the generic surface** | |
| Docker distributed-sync tests, `three_piece_capsule`, `tcp_static_peer_sync` | **Keep; extend with the generic estimation fixture** | Blanket §6.2 |

---

## 3. The consolidated model

What is left, stated once, in the order it should appear in the rewritten spec. The
spec's existing prose is reused wherever it survived; this is the skeleton.

### 3.1 Concepts

1. **Piece** — a device with a cryptographic identity. Signs what it writes.
2. **Capsule** — one person's pieces; equal access within; retired whole; the unit of
   custody and consent. Apps never see it.
3. **Flow** — a UUID-identified, typed bundle of streams with per-stream policies. One
   flow per set of streams that must be fitted together invisibly. Created by a piece,
   owned by its capsule.
4. **Stream** — named, kinded conduit inside a flow. Four kinds:
   - **log**: per-party, append-only, sequenced, replayable, never merged;
   - **jet**: per-party, lossy, windowed;
   - **drip**: keyed convergent state; either per-party keyed (each party owns its keys;
     `ConvergentDocument`) or role-written (one writer under lease);
   - **query-response**: served by a role, answers requests.
   A **summary** is a drip whose only readers are another flow's role holders.
5. **Record** — one unit on a stream. Opaque bytes to the protocol. Every record has an
   author (piece), and every *derived* record cites the **horizon** of the streams it was
   computed from.
6. **Role** — a named responsibility in a flow (source, sink, memorization, fitting,
   curator, aligner, or type-specific). Filled by parties. Exclusive roles are held under
   a **lease** chosen by the **delegation policy** (capability, standing, authorization
   coverage, completeness).
7. **Fitting** — what a role's code does between streams. Not an object.
8. **Custody** — where records rest. **Origin custody** is the author's, governed by the
   author's capsule, untouched by any flow policy or revocation. **Replica custody** is
   governed by the stream's memorization policy (retention; at-rest tier open / sealed /
   dissolved; who may replicate) and ends with discard when a party leaves.
9. **Grant** — a capsule's permission to a piece, a capsule, or a role to read, write, or
   serve one stream of one flow. Implicit and total within a capsule; explicit, per
   stream, revocable across capsules. Requires a prior **introduction** between
   capsules. Presented to the person through the **auth surface** inside whatever app
   needs it.
10. **Reference** — a flow holding another flow's UUID, optionally with a grant on a
    named stream of it. Without the grant it is an **opaque reference**: displayable,
    not openable. The mesh is references.
11. **Derivation** — the only thing that crosses a capsule boundary is a record the
    owner's own fitting produced for that audience: a summary, a projection, a public
    cache entry. Never the raw stream. (V: all data is derived.)
12. **Ensemble** — the live reachable set of a capsule's pieces. **Flow ensemble** — the
    live reachable set of a flow's parties across capsules. Presence, lease lapse, and
    routing for shared flows happen here.
13. **Routing fabric** — logical addressing with TTL envelopes over any transport;
    BLE-first; relays never inspect payloads.
14. **Clock reference** — per-ensemble offset estimates with uncertainty, exposed to
    applications, which write timestamps into their own records.
15. **App surface** — session, flow lifecycle, streams by name and kind, roles, presence,
    clock, access requests, and a closed error set. Language bindings expose exactly
    this.

### 3.2 Invariants

- **C1 Origin custody is inviolable.** No policy, grant, or revocation reaches what a
  piece produced and holds in its own capsule.
- **C2 Replicas discard on leave.** Losing a role, a grant, or membership discards
  replica custody of the affected streams; the protocol does it and reports it.
- **C3 Only derivations cross capsules.** A flow may read another flow's raw streams
  only within one capsule. Across capsules it reads granted summaries and projections.
- **C4 One flow per fitting necessity.** Streams that must be fitted together are in one
  flow; sharing is per stream and by reference, never by shrinking flows.
- **C5 Derived records cite their horizon.** Reproducible, auditable, and loop-proof by
  construction (raw-stream types have no field for a derived reference).
- **C6 Public caches hold nothing privacy-relevant.** A cache entry that exists to make
  a shared view responsive (a 4×4 colour grid and an aspect ratio; a dependency shape
  and a duration; a frame's existence and motion class) is a derivation chosen so that
  its disclosure is not a disclosure. Titles, text, pixels, trajectories never qualify.
- **C7 Revocation is promised honestly.** It stops future reads and writes, rotates
  keys, and discards replicas. It does not recall a derivation someone already holds.
  Every UI says exactly this.
- **C8 Apps never see capsules.** Identity questions an app may ask: "who am I" and
  "same capsule as me?" Nothing else.
- **C9 Validation after materialisation.** Convergence produces a state; the
  application judges its validity; the protocol never enforces application invariants
  at write time.

Every one of these maps to a clause in the substrate contract on the other side of the
blanket, and each will have a conformance test.

---

## 4. The documentation cascade

In order. Each step leaves `docs/INDEX.md` accurate.

### 4.1 The spec (`rim-self-data-flows.tex`)

Restructure, reusing existing prose:

1. Introduction: keep. Rewrite "Relationship to Existing Code" as a two-line pointer
   to §5 of this document; it will be stale again in a month otherwise.
2. Self-Data Flows: keep; demote content-hash typing to "intended, not implemented."
3. Streams: rewrite around the four kinds and the summary (§3.1.4). Move the drip/jet
   philosophy paragraphs under the drip and jet kinds.
4. **Custody** (new; replaces the memorization parts of Security and Memorization):
   origin vs replica, per-stream policy table, tiers, dissolution, robustness, discard
   on leave.
5. Flow Roles and Fittings: keep; add lease and delegation eligibility here or in
   Policies (one place only).
6. Policies: error (closed set), delegation, discovery, retirement. Memorization moves
   to Custody.
7. **Grants and Introduction** (new; replaces Authentication and Authorization):
   intra-capsule implicit; inter-capsule per stream; introduction; revocation with
   rotation; the auth surface; what apps may ask.
8. **Privacy and Disclosure** (new): the values (V) in the spec's own words; C3, C6, C7;
   derivation; nesting as disclosure; public caches; unlinkability stated with its
   limits (content-addressed media, physical fiducials).
9. Flow Boundaries and Mesh: keep the principle; replace the inventory example;
   references, opaque references, nesting (already added).
10. Data Type Examples: keep Images (rewrite the album tier as a collection drip +
    public cache), Photo Albums, Inventory (rewrite per §6.1), Giantt (rewrite per
    §6.2), Text (rewrite as the notes example per §6.3), Spatial Measurements (done),
    Sound and Video as sketches. Retire Vector Images and Binary Data as separate
    examples (binary is the asset flow inside Images).
11. Device Topology: keep; flow ensembles (added); clock reference (new paragraph).
12. **Application Surface** (new): §3.1.15 with the closed error set and the binding
    rule.
13. Appendices: Data Geometry (moved); Terminology (regenerated); retire the Twigs
    comparison (the system it compares to is itself retired).

### 4.2 Other design documents

| Document | Action |
|---|---|
| `authorization-model.md` | Rewrite as the source for spec §7; keep the Link model prose verbatim |
| `convergent_document_design.md` | Keep; add one section: "as the engine of the per-party keyed drip" |
| `transport_architecture.md`, `transport_tiers.md` | Keep; add the clock reference and flow-ensemble presence |
| `app_soradyne_boundary.txt` | Fold its two decisions into spec §12 and retire the file |
| `capsule-ensemble-implementation-plan.md` | Retire; phases 0–6 are history in git, phase 7–8 are superseded by §5 |
| `capsule-imp-plan-notes.rtf`, `tcp_emulation_of_ble_plan.txt`, `tcp-sync-analysis.md`, `20260312_giantt_sync_thoughts.txt` | Retire; all marked obsolete or superseded in the index already |
| `20260729_flow_sharing_granularity_{conversation,takeaways}.md` | Retire with a one-paragraph tombstone in the index pointing at the two assessments and this cascade; the decisions they record are reversed here |
| `20260906_fable_*` (assessments, mapping, estimation flows, blanket, this) | Keep until the spec rewrite lands; then retire the two assessments and keep the other three |
| `INDEX.md` | Keep; it is doing its job |
| `CLAUDE.md` | Rewrite: drop Phase History and the pairing-bridge internals; describe the consolidated model, the blanket, the app surface, and the build commands |

### 4.3 Giantt and inventory design notes

`giantt-design-notes/` stays authoritative for giantt semantics; add a short note
recording the relation decision of §6.2. `inventory-design-notes/` (if re-homed) gains
the spaces model of §6.1.

---

## 5. The implementation cascade

Order matters; each step is testable alone, and the blanket's stratum 6.2 fixture
(`estimation_flows.md` §1, the toy "room temperature" application) is the running test.
Agents fill in code detail; this is the shape.

1. **Streams as kinds.** In `flow/`: a `LogStream` (per-party journal + sequence +
   replay + horizon, extracted from `drip_hosted.rs`), a `JetStream` (windowed,
   drop-on-backpressure), a `KeyedDrip` (`ConvergentDocument` behind a register API),
   a `RoleDrip` (register with lease-gated writes), a `QueryStream` (request/serve).
   `DataChannel` remains the in-memory transport for all of them.
2. **Flow types as stream sets with per-stream policy.** `FlowSchema` gains kind and
   custody policy per `StreamSpec`; `FlowRegistry.load` builds the streams. The
   estimation-scope and instrument flow types are the first two registered from a
   design doc rather than hand-written.
3. **Custody.** A `custody/` module: retention timers, at-rest tiers (open, sealed via
   capsule keys, dissolved via the existing erasure code), replica bookkeeping, and
   discard on leave. Origin custody is just "the author's own journal."
4. **Delegation and leases.** Revive Phase 5 host assignment as a general `Lease` over
   the flow ensemble: claim, renew, standby notification, lease-carried writes.
5. **References and opaque references.** A flow config may reference other flows; open
   without grant yields an opaque handle.
6. **Flow ensembles.** Presence and routing across capsules over the existing envelope
   fabric; first over TCP/overlay, BLE later.
7. **Grants and introduction.** The inter-capsule design: per-stream keys, invite and
   accept, rotation on revoke, the auth surface. This is the largest new piece and the
   last that the estimation application can wait for (its first use case is single
   capsule).
8. **Clock reference.** Round-trip offset estimates on ensemble links.
9. **App surface.** Finish `soradyne_flow_*` into the closed API; remove the capsule id
   from `connect_ensemble`; add a PyO3 crate exposing exactly the same surface; regenerate
   the Dart client from it.
10. **CLI.** Stream-level commands (`flow streams`, `stream tail`, `stream replay`,
    `lease status`, `custody status`).
11. **Retire** per §2.7 and §7, as each replacement lands.

Tests at each step: the generic fixture over sim BLE in-process, then over Docker with
the existing scenarios extended, then the conformance suite from across the blanket.

---

## 6. The applications, re-derived

The rule for all four: the app's own data model stays the app's; soradyne carries
records and enforces custody, grants, and derivation. Each app gets a short flow layout,
a sharing model, and a migration note.

### 6.1 Inventory: spaces

**Requirement.** Items are mostly mine; some are stored in spaces other people use (the
workshop); each space has a description independent of the items in it; sharing differs
per space by who uses that space; moving an item between spaces must not destroy its
identity or history.

**Layout.**

- `inventory.items` flow, one per capsule: a per-party keyed drip. Key = item UUID.
  Record = the full item (category, description, photos as references, custom fields,
  history). This is the person's inventory, all of it, at home.
- `inventory.space` flow, one per space (my room, the workshop, the storage unit).
  Streams: `description` (a per-party keyed drip of prose blocks, or a text flow
  reference), `contents` (a role-free keyed drip: key = item UUID, record = the owner's
  **projection** of that item for this space), `viewers` (jet, optional).
- A space owned by one capsule is private; a space with contributors from several
  capsules is shared. The workshop is owned by whoever created it; Dana and Jaguar both
  hold write grants on `contents` and read grants on `description`.

**Placing an item in a space** = the owner writing a projection of it into the space's
`contents` under the item's UUID. The projection is chosen per space: at the workshop,
name, photo public-cache, quantity; at home, everything. **Moving** = writing into the
new space and tombstoning in the old. The item's own record at home is untouched, keeps
its UUID and its history, and gains a "location" field pointing at the space's flow
UUID. Continuity is a property of the record, not of a flow. Mass-flow patterns are
computed at home from the item records and their location history.

**Sharing** = grants on the space's streams. Jaguar sees the workshop's `contents`,
which holds only what each contributor projected there. He never sees anyone's
`inventory.items`. Revoking Jaguar = C7: he keeps whatever projections he replicated
while granted; nothing further, and his replica custody of the space is discarded.

**The living-space description** is the `description` stream of the home space,
independent of items, private to the capsule, and consumable by the agent that plans
moves because that agent runs inside the capsule. It wants a prose-capable drip: either
a block-keyed drip (paragraph = key, latest-wins per block) or the text flow of spec §Text
once that exists. Block-keyed is enough for a description.

**Migration.** The single hard-wired flow UUID becomes `inventory.items`; every item
gains a location field; one `inventory.space` (home) is created with the existing
description; the workshop is created when it is shared. The app's `InventoryApi` becomes
multi-flow (the Ghost analysis already established the Rust side supports this).

### 6.2 Giantt: charts, and the relation decision

**Requirement.** Items are private by default, editable by several people with one
coherent result, meaningful mainly through relations; charts are shared with different
people; overlaying charts is the primary view; the critical path must be visible; a
`REQUIRES` edge asserts necessity.

**The decision.** Relations split by kind, and the answer is obvious once derivation is
the sharing mechanism:

- **`REQUIRES`, `ANYOF`, `BLOCKS` are intrinsic**: they are facts about the item and live
  in the item's record in its owner's flow. Sharing an item into a chart projects the
  item *with* its intrinsic edges; an edge to an item not projected into that chart
  appears as an **opaque reference** ("depends on something you cannot see"), which is
  truthful and matches the mesh rule. The semantics note's necessity test is honoured.
- **`SUPERCHARGES`, grouping, ordering, layout, colour, chart membership are extrinsic**:
  they live in the chart's flow.

**Layout.**

- `giantt.items` flow, one per capsule: per-party keyed drip. Key = item UUID. Record =
  title, status, duration, notes, time constraints, intrinsic edges, log entries.
- `giantt.chart` flow, one per chart: `members` (keyed drip: key = item UUID, record =
  the owner's projection: dependency shape, duration, status, and title only if the
  owner grants it, per C6), `extrinsic` (keyed drip of edge tuples `{from, to, type}`),
  `layout` (keyed drip), `viewers` (jet).
- Overlay = union of projections and edges across selected charts, validated after
  materialisation (C9; `GraphDoctor`). Critical path is computed over the overlay and
  labelled as such. Cycles across charts are reported, not prevented. The personal
  "all my items" view is a chart the app synthesises from `giantt.items` directly, so
  chart-agnostic CLI commands have a well-defined scope: the owner's items.

**Migration.** Item IDs become UUIDs with the human id kept as a field; bidirectional
relations collapse into one intrinsic edge per pair on the source item; the `charts` set
on items inverts into chart `members`; `SUPERCHARGES` edges move to charts (into the
chart where both endpoints are members, else the personal chart).

### 6.3 kmeep: notes for friends and family

**Requirement.** A Keep clone: notes with title, body, checklists with two levels,
attachments at the top, pinned and unpinned, labels by tag, save-as-you-go, tens of
thousands of notes, no merges needed ("just overwrites"), history optional and
separable. Already has one sharing rule: notes are visible to an agent by `#rd`/`#no-rd`
tags. Soon: released to friends and family, with the sharing and privacy a notes app
must have.

**Fit.** kmeep is the spec's Text example ("start big with file trees") plus the Images
example for attachments, and its storage provider interface (`noteStore.js`) is already
the seam. Its design note says the file on disk is the stateless source of truth with
no hidden ids. Under soradyne the record key is the note's UUID *inside the flow*; the
file-tree **projection** (filename = title, pinned at root, unpinned in a folder,
attachments as wikilinks) is a derivation the app renders for Obsidian, so the vault
stays isomorphic and id-free while the flow has stable identity underneath.

**Layout.**

- `notes.mine` flow, one per capsule: per-party keyed drip. Key = note UUID. Record =
  title, body (markdown), pinned, tags, attachment references, created, modified. Latest
  wins per note, which is what the design asked for.
- `notes.shared` flow, one per **shared note**: created when a note is first shared.
  Streams: `note` (per-party keyed drip with the note's fields as keys, so two people
  editing different fields converge; same-field edits are latest-wins, Keep-style),
  `comments` (log per party), `viewers` (jet). The note's UUID is the same as at home;
  its home record gains a "shared as" reference. Un-sharing = retiring the shared flow
  (C7 stated in the UI: "they keep the version they had").
- Attachments: `image` flows (query-response by resolution tier) owned by whoever
  attached them, referenced from the note, with a **public cache** entry (the 4×4 grid
  and aspect ratio) so shared notes lay out instantly and resolve from the owner's
  devices. Content-hash deduplication stays *inside* a capsule and is never the
  cross-capsule identifier (unlinkability decision recorded).
- **Agent access** generalises the `#rd` rule: the responsible agent is a party in the
  capsule with a *read grant computed from tags*. That is a grant rule, not a filter,
  and it means the same mechanism gives "share this label with my partner" for free.
- History, if built, is a log stream of note snapshots per party, separable by
  retention policy, and rendered into a history folder only on request.

**Migration.** Import from the Obsidian folder: one record per file, UUID minted at
import, title and pinned from filename and folder, attachments imported as image flows.
The Obsidian projection keeps working as an export for as long as Dana wants both.

### 6.4 Photos: how the original app fits, and does not

The album system was soradyne's first application and shaped the crate. Against the
consolidated model:

**What fits.** The spec's three-tier decomposition (image flow, composite flow, album
flow) is exactly a query-response asset flow, a keyed drip of edit operations, and a
collection drip of references. The dissolution storage is the `dissolved` custody tier.
The renderer's resampling is how derivations (tiers, public-cache grids) are computed.
Comments and reactions are per-party logs. Who-is-viewing is a jet.

**What does not.** The album module has its own CRDT traits, its own sync manager and
message protocol, its own FFI, and content-hash media addressing as the sharing
identifier. It predates flows and does not use them. Its identity model has no
capsules and no grants. Its unlinkability story is the opposite of the vault's.

**Verdict.** Retire the module and the demo app's album screens; rebuild photos as the
first *pure* application of the consolidated model, because it exercises every part:
asset flows with tiers, public caches (V's own worked example), collections with
opaque references, per-party logs, jets, sharing with revocation-you-can-feel, and
dissolution for originals. Keep it small: album, photos, comments, sharing with one
friend. It becomes the spec's Images and Photo Albums example made real, and it is also
what kmeep's attachments need.

---

## 7. Retire list for the garden pass

History lives in git. Recommend removing from the tree, each with a one-line tombstone
in the index or CLAUDE.md pointing at the commit range:

**Code**

- `packages/soradyne_core/src/album/` (all), `src/types/` (all), `src/video/`,
  `src/bin/`, `examples/` (all eleven), `src/flow/examples/`, `src/network/` (after
  folding discovery into `ble/`), the album half of `src/ffi/mod.rs`.
- `apps/soradyne_demo/` in its current form: keep only the pairing and capsule screens
  as the seed of the rim device-management app; the flow-demo and album screens go.
- `apps/kmeep/` in this repo: a stub with backup files; the real app is in the vault.
  Either move the real app here when it moves to soradyne, or delete the stub.
- `packages/ai_chat_flutter/`: out of scope per Dana; leave as is.

**Docs**

- `capsule-ensemble-implementation-plan.md`, `capsule-imp-plan-notes.rtf`,
  `tcp_emulation_of_ble_plan.txt`, `tcp-sync-analysis.md`,
  `20260312_giantt_sync_thoughts.txt`, `app_soradyne_boundary.txt` (after folding),
  `20260729_flow_sharing_granularity_{conversation,takeaways}.md` (after tombstone),
  the prior-system-comparison appendix of the spec, `nestbox-example-system-usage-symlink` (a
  dangling macOS path).

**Keep and rebuild on the new foundation** (not retire): pairing, capsules, sim BLE,
btleplug and Android transports, TCP static peers, envelope routing, `ConvergentDocument`,
dissolution storage, the CLI, the Flutter plugin, the Docker tests.

---

## 8. Sequencing and definition of done

Milestones, each verifiable:

1. **Docs**: spec restructured per §4.1; index accurate; CLAUDE.md rewritten; retire
   list applied. No code yet. *Done when* the spec reads as one system and the blanket's
   parity ledger has a counterpart for every substrate clause, even if marked "gap."
2. **Streams and custody**: §5 steps 1–3; generic fixture passes in-process and over
   Docker. *Done when* the log, jet, keyed drip, role drip, and query streams each have a
   test and custody discards on leave.
3. **Leases, references, flow ensembles, clock**: §5 steps 4–6, 8; conformance suite
   passes for a single capsule. *Done when* the basement use case across the blanket
   runs on real hardware.
4. **Grants and introduction**: §5 step 7; conformance passes multi-capsule. *Done when*
   the shop-cell use case runs with two capsules, and the workshop inventory space is
   shared with one friend.
5. **Applications**: inventory spaces, giantt relation split, photos rebuilt, kmeep
   provider. *Done when* each app's sharing story can be demonstrated with revocation
   and the UI states C7 verbatim.

The order of 4 and 5 can interleave: inventory spaces and giantt charts within one
capsule need only milestone 3.

---

## 9. What this document does not decide

- The wire format of grants and the key schedule for rotation (milestone 4 design).
- Whether the text flow is block-keyed or a sequence CRDT (kmeep and the living-space
  description are fine with block-keyed; a collaborative long document is not).
- The exact public-cache derivation per type (a 4×4 grid is the photo example; the
  giantt and notes equivalents need a sentence each in the Privacy section).
- Anything about the rim device-management app's UI beyond "it is where capsules are
  managed and it is never the entry point."
