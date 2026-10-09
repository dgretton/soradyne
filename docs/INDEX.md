# Documentation Index

Index of files in `docs/`, with approximate dates, content summaries, and obsolescence notes.

## Current architecture (2026-09-11)

### Information lifecycle investigation (2026-10-07; closing scope 2026-10-08)
- **Location:** [research plan](../research/information_lifecycle/PLAN.md),
  [status](../research/information_lifecycle/STATUS.md), and
  [methods](../research/information_lifecycle/METHODS.md).
- **Status:** isolated research on `research/information-lifecycle`, based on
  `shared-flow-demo-contracts`; not production behavior or a revised protocol spec.
- **Scope:** preserve future mathematical options through representation, retention,
  identity and contracts. Near-term deployment uses one local network or Tailscale, with
  unavailable nodes repaired before sessions. At most four further cycles after 0011:
  I07/R05 calibration and withdrawal, I08/S02 snapshots and reset semantics, F04 crash
  boundaries, then synthesis and independent architecture/mathematics reviews. Other
  work remains explicitly deferred without being ruled out by early design choices.
  Owning contract changes stay proposals; pause automation after the reviews are pushed.
- **Latest evidence:** [cycle 0011](../research/information_lifecycle/cycles/0011.md)
  tests repeated-sample angular coverage on 131,072 seeded datasets. Local Gaussian
  intervals under-cover in weak geometry; full circular intervals pass the declared
  primary/held-out checks with a model-specific justification, but remain broad.
  Frozen-anchor dependence and a numerical-CDF limitation are documented. The queued
  converged-freeze/new-batch experiment is cancelled under the narrowed scope;
  general survey marginalization and consumer safety remain unproven.

### `20260911_shared_flow_demo_contracts.md`
- **Status:** current design direction; implementation and demo fixtures pending.
- **Content:** one shared concept per flow, independent ownership/roles, plural
  capsule participation, opaque cross-capsule forwarding, substantial distributed
  retention, selective withdrawal, policy-defined fitting acceptance, and F1-F7
  scenarios including mixed fast interaction and archival retry.
- **Precedence:** supersedes conflicting assumptions in the cascade, estimation-flow
  note, authorization model, transport architecture, and protocol prose. Existing app
  layout proposals remain proposals. The full protocol prose/PDF is not regenerated;
  its source includes a visible revision notice.

---

## rim Protocol Specification

### `rim-protocol/rim-self-data-flows.tex` (.pdf, Makefile)
- **Written**: ~Feb 2026
- **Last modified**: Sep 6, 2026 (example-style additions for distributed estimation; see `20260906_fable_estimation_flows.md`)
- **What it is**: The primary design document for the rim Self-Data Protocol. 1,966 lines of LaTeX defining self-data flows as the core abstraction: flows, streams (drips and jets), data geometry, flow roles, fittings, policies, security/memorization, and device topology (capsules, ensembles, parures). Includes worked examples for images, video, sound, spatial measurements, photo albums, binary data, inventory, task graphs (Giantt), and text. TikZ diagrams. Appendices comparing with a prior bespoke sensor-routing system and listing implementation status.
- **Status**: **Mostly current as design intent.** The conceptual model (flows identified by UUID, typed by content-hashed design documents, composed of streams, operated by roles, governed by policies) remains the target architecture. The "Relationship to Existing Code" section (line 228) is accurate: `DataChannel<T>` is a stream not a flow, `ConvergentDocument<S>` backs a drip, the `Flow` trait is directionally correct. The stale implementation status appendix was removed (Apr 2026). The flow type system described (content-hashed versioned type documents, `rim.inventory.v1` naming) is more elaborate than what's implemented (simple string names like `"giantt"`, `"inventory"`). Per-stream authorization between capsules is described but not designed in detail.

---

## Implementation Plans

### `capsule-ensemble-implementation-plan.md`
- **Written**: Feb 13, 2026
- **Last modified**: Feb 18, 2026
- **What it is**: Detailed 8-phase implementation roadmap (105 KB). Covers cryptographic identity (Phase 0), BLE transport (Phase 1), capsule data model (Phase 2), ensemble discovery (Phase 3), pairing UX (Phase 4), DripHostedFlow (Phase 5), Giantt/Inventory sync demo (Phase 5.5), integration testing (Phase 6), photo flows (Phase 7), and PickPlaceFlow (Phase 8). Includes appendices on crate layout, dependency inventory, and open questions.
- **Status**: **Phases 0–6.3 are implemented; the plan served its purpose for those phases.** The code now diverges from the plan in details (e.g., the plan's API signatures don't always match what was built, the `FullReplicaFlow` naming replaced `DripHostedFlow` internally in some places). Phase 7 (post-pairing persistent BLE, photo flows) and Phase 8 (PickPlaceFlow) remain unbuilt and the plan is still relevant there. The plan assumes apps know about capsules directly (e.g., `soradyne_flow_connect_ensemble(capsule_id)`), which is now considered a layer violation — future work should decouple apps from capsule awareness.

### `capsule-imp-plan-notes.rtf`
- **Written**: ~Mar 10, 2026
- **Last modified**: Mar 10, 2026
- **What it is**: Design commentary in RTF. Raises questions about piece vs. device identity naming, BLE central/peripheral topology (answered: every BLE connection is one central + one peripheral, this is a built-in BLE property), Flutter Bluetooth dependency concerns (answered: Rust now owns all BLE via btleplug + JNI, Flutter doesn't touch the radio), and Tauri/VR/C# cross-platform considerations.
- **Status**: **Largely resolved.** The core questions have been answered by implementation: Rust owns BLE, `btleplug` handles macOS/Linux central, JNI handles Android peripheral, "soradyne apps don't touch the radio" is the guiding constraint. The Tauri parallel demo idea remains unbuilt but is still relevant as a modularity check. VR/C# interop is unaddressed. The piece-vs-device naming discussion is still somewhat live.

---

## Design Documents

### `authorization-model.md`
- **Written**: Apr 13, 2026
- **Last modified**: Apr 13, 2026
- **What it is**: Writeup of the two-domain authorization model for rim. Distinguishes intra-capsule auth (one person, many devices -- all pieces get equal access, apps never see capsules) from inter-capsule auth (many people, shared flows -- per-stream E2E encryption, not yet designed). Documents the "Link model" for app integration (embedded auth surface, no "install rim first" requirement), the naming discipline (capsule/piece are internal-only terms), and implications for the current FFI surface.
- **Status**: **Current.** Captures design decisions made Apr 2026. Inter-capsule auth is described as an open problem, not a solved design.

### `convergent_document_design.md`
- **Written**: Feb 8, 2026
- **Last modified**: Feb 8, 2026
- **What it is**: Design spec for `ConvergentDocument<S>`, the CRDT engine. Defines the five primitive operations (AddItem, RemoveItem, SetField, AddToSet, RemoveFromSet), informed-remove semantics, causal horizon tracking, state materialization (latest-wins for scalars, additive merge for collections), lazy hash-based compaction, and schema definition.
- **Status**: **Current.** The implemented `ConvergentDocument` closely matches this design. The five operations, horizon tracking, and materialization rules are all in the code. The SelfDataFlow integration pattern shown (apply_local + publish / subscribe + apply_remote) is the intended pattern. `FullReplicaFlow` now handles persistence internally via per-party journals and uses horizon-based incremental sync, so the integration pattern described here is realized in practice.

### `transport_tiers.md`
- **Written**: Mar 12, 2026
- **Last modified**: Mar 12, 2026
- **What it is**: Architecture document for multi-tiered transport. Tier 1: in-process (SimBLE + LAN TCP). Tier 2: LAN (mDNS/DNS-SD + TCP). Tier 3a: WAN via mesh VPN overlay. Tier 3b: WAN native (STUN + hole punching). Tier 3c: global relay fallback. Maps BLE operations to LAN/WAN equivalents.
- **Status**: **Current as design intent; only Tier 1 is implemented.** SimBleNetwork and static-peer TCP (a simplified variant of Tier 1b) exist. Tiers 2–3 are unbuilt. The mDNS mapping table and encrypted-advertisement-in-TXT-record design are still the plan.

### `transport_architecture.md`
- **Written**: Apr 24, 2026
- **Last modified**: Apr 24, 2026
- **What it is**: Comprehensive transport architecture document covering principles that span multiple areas: BLE as the universal abstraction for all networking (with TCP, CAN bus, LoRa as backends behind the same traits), transparent transport switching, the embedded engine model (soradyne runs within apps via FFI, no daemon required), TCP multiplexing challenges (emulating BLE's OS-managed connection sharing), event-driven design (sync triggered by actions, not polling), the discovery progression (static peers → mDNS → STUN/rendezvous → relay at rimm.ing), security invariants, and the relationship between `soradyne-cli` daemon and app-embedded soradyne.
- **Status**: **Current.** Captures design principles and constraints as of Apr 2026. References `transport_tiers.md` for the detailed tier plan. The TCP multiplexing section identifies an open design problem (multiple app processes sharing a TCP listener port to emulate BLE's OS-managed multiplexing).

### `app_soradyne_boundary.txt`
- **Written**: Apr 24, 2026
- **Last modified**: Apr 24, 2026
- **What it is**: Raw design notes (stream-of-consciousness) resolving two related architectural questions: (1) where app-specific schema code belongs — soradyne's `ConvergentDocument` should be generic, schema knowledge stays in the app layer, giantt/inventory schemas should not be baked into soradyne_core; (2) how flow IDs are owned — soradyne originates them on request but the app holds them as opaque pointers, apps keep flow IDs in their own storage, and multiple devices sharing a flow ID is the entire basis for sync. Also sketches the app-side integration model: init soradyne generically, pull flow IDs from app storage, open flows by ID, write ops and read materialized state. Notes the many-to-many relationship between user-facing "personas" (multicharts) and flows, and the dev-stub pattern for seeding a shared flow ID across monorepo builds.
- **Status**: **Partially implemented; good forward reference.** The two core decisions are in place: `GianttSchema`/`InventorySchema` removed from soradyne_core (Apr 2026), and `GianttService` stores flow IDs in app documents with a dev-stub. The forward-looking material — persona/flow many-to-many, flow splitting for selective sharing, inter-capsule flow sharing, full app initialization model at scale — is not yet built. This document is a useful reference for the long-horizon design of the soradyne/app layering and how things will work at full complexity.

### `tcp_emulation_of_ble_plan.txt`
- **Written**: Apr 24, 2026
- **Last modified**: Apr 24, 2026
- **What it is**: Raw design notes from Dana articulating the transport architecture principles: TCP backing BLE transparently, the embedded engine model, per-process soradyne instances, TCP multiplexing challenges, event-driven design, and the relationship between the CLI daemon and app-embedded soradyne. Stream-of-consciousness format.
- **Status**: **Superseded by `transport_architecture.md`**, which distills and contextualizes the same ideas. Retained as the original articulation.

### `20260729_flow_sharing_granularity_conversation.md`
- **Written**: Jul 29, 2026
- **Last modified**: Jul 29, 2026
- **What it is**: Verbatim transcript of a high-level design conversation between Dana and Claude working through how selective sharing *between people* interacts with the flow abstraction. Starts from the inventory-sharing problem (some items mine-only, some shared with Jaguar at the rim workshop, moving items without destroying their continuity) and derives a general model. Key threads: logical atomization vs. physical aggregation of flows, the item-is-the-flow continuity model, collection flows (the generalized photo-album pattern), extrinsic-only giantt relations (edges live on charts, not items), access transitivity across the flow mesh, and a bookmarked coherence/synchronization primitive. Companion analysis: `20260729_flow_sharing_granularity_takeaways.md`.
- **Status**: **Currency medium-high, settled-ness low.** Grounded in the current protocol spec (`rim-self-data-flows.tex`) and `authorization-model.md`, and consistent with the Flow Boundary / Flow Mesh sections. But this is an in-progress design dialogue, not a spec: two decisions were taken provisionally (bless the collection flow type; giantt relations extrinsic-only), several conclusions depend on **inter-capsule auth, which is still undesigned**, and nothing here has been written into the spec or the code. Read as a snapshot of live thinking, not settled architecture.

### `20260729_flow_sharing_granularity_takeaways.md`
- **Written**: Jul 29, 2026
- **Last modified**: Jul 29, 2026
- **What it is**: Structured distillation of the conversation transcript above: decisions taken (collection flow blessed as first-class; giantt relations extrinsic-only; coherence primitive bookmarked to the collection type), the core conceptual model (logical atomization / physical aggregation, item-is-the-flow continuity, graduated disclosure via cached hints, the two-axis split rule), seven known issues to design around (convergence≠validity, chart-relative derived state, cache discipline for hints, partial aggregate queries, structured CRDT set elements, migration, access transitivity), and five open questions (collection flow type definition, inter-capsule auth, coherence groups, auto-grant-vs-prompt, parked intrinsic relations).
- **Status**: **Currency medium-high, settled-ness low.** A working note, not a spec. Explicitly records that `rim-self-data-flows.tex` is unmodified, no code has changed, and inter-capsule auth remains undesigned. The proposed next work item is drafting the blessed collection flow's type definition. **Critiqued** (Sep 2026) by the two `20260906_fable_flow_granularity_assessment_*.md` documents below; Issue F is stale against the code (`Value` already has `List`/`Map`) and Decision 2 is contested by `giantt-design-notes/giantt-dependency-and-timing-semantics.md`.

### `20260906_fable_flow_granularity_assessment_concepts.md`
- **Written**: Sep 6, 2026
- **Last modified**: Sep 6, 2026
- **What it is**: Critique of the flow-granularity plan (the 20260729 takeaways) at the protocol level, written by Claude Fable 5.1 after reading the full spec, auth model, and code. Seven contradictions with `rim-self-data-flows.tex`: the plan substitutes shareability for fitting-necessity as the Flow Boundary determinant (the spec's own atomic-unit example is "an inventory's items"); the coherence-group bookmark duplicates what a flow already is and what per-stream auth already gives; ensembles are scoped to one capsule so shared flows have no transport to run on; per-flow configuration/party lists do not collapse under multiplexing; one-file-per-peer journals collide with per-stream keys and revocation; "UUIDs sufficient to bootstrap access" becomes false; multi-level references are unaddressed. Six internal tensions: graduated disclosure is an auth rule not a free capability; location-as-reference vs location-as-field; the missing root collection; a second sense of partial aggregates; ownership needed by the sharing UX violates the Link model; Issue F is stale. Ends with what survives and a five-item resolve-first list.
- **Status**: **Currency high, settled-ness low.** A critique, not a decision record. Nothing in the spec or code changed. Should be read alongside the takeaways before the collection flow type is drafted.

### `20260906_fable_flow_granularity_assessment_giantt.md`
- **Written**: Sep 6, 2026
- **Last modified**: Sep 6, 2026
- **What it is**: Giantt-specific companion to the concepts assessment. Argues that extrinsic-only relations (Decision 2) contradict giantt's own edge semantics — the 2026-05 session with Jaguar defines a REQUIRES edge as counterfactual necessity, which is an intrinsic fact — and proposes a split by relation type (REQUIRES intrinsic, SUPERCHARGES/ordering extrinsic). Further tensions: the migration's personal chart reintroduces the global graph the plan renounces; overlay is giantt's primary view but the plan makes it the inconsistent edge case; cross-chart edges are routine (promotion pattern) and chart membership vs edge endpoints is undefined; chart-relative derived state conflicts with chart-agnostic CLI commands; the migration is not trivial (string IDs to UUIDs, bidirectional dedup, membership inversion); hints on shared charts have two writers on one latest-wins field. Ends with a five-item decision list.
- **Status**: **Currency high, settled-ness low.** Depends on the concepts assessment and on `giantt-design-notes/giantt-dependency-and-timing-semantics.md` (which was untracked when this was written).

### `20260906_fable_nestbox_flow_mapping.md` — **moved out of this repo**
- **Written**: Sep 6, 2026
- **Last modified**: Sep 6, 2026
- **What it is**: Design proposal mapping a companion coordinate-alignment project's ground-up rebuild (networked coordinate-frame alignment: frames, factors, elected aligners per solver domain, epochs, `lookup()` with joint covariance) onto rim self-data flows, assuming multiple people/capsules contribute instruments and data. Core mapping: a solver domain is a flow; each node's factors are a per-party append-only edit stream (`factors`) plus a per-party jet for operation-mode observations (`factors_live`); solved edges with epochs are the domain drip (`edges`), carrying the factor horizon they were computed from; `lookup` is a query-response stream served by the delegated `aligner` role; boundary edges go out on an `exports` drip that is the only stream a parent domain may read. Rigid bodies/instruments are separate longer-lived per-body flows referenced by domain flows; appearance models, a site index (collection flow), and a private per-node recording flow round out the set. Domain nesting is identified as the privacy boundary (a headset or bedroom is a private child domain; shared domains ingest exports only), which makes the companion project's invariant 8 and rim's per-stream authorization the same rule. Includes a transport/discovery/election/identity/time mapping table, a resolution of the flow-granularity tensions in the companion project's favour (stream is the unit of sharing; domain = flow by fitting necessity; no coherence groups; site index is the root collection), a list of eight soradyne gaps in build order (Python binding + generic `flow_open`, CRDT-free per-party log streams, jets, query-response streams, delegation with eligibility rules, multi-capsule flow ensembles, time, large assets), a transport-neutral contract so the companion project never names rim, and walk-throughs of three physical-space use cases.
- **Status**: **Relocated, Sep 6, 2026.** This file names the companion project concretely throughout (mapping table, stream names), which the blanket-discipline doc below rules out for anything committed in this repo, so it was moved to `~/Dropbox/Personal Projects/Nestbox/20260906_fable_nestbox_flow_mapping.md` on this machine rather than genericized in place. The soradyne-facing generic counterpart that stays in this repo is `20260906_fable_estimation_flows.md` below. No longer present in this repo.

### `20260906_fable_estimation_flows.md`
- **Written**: Sep 6, 2026
- **Last modified**: Sep 6, 2026
- **What it is**: Protocol requirements for the class of application whose data is many parties' observations fused by a designated party into an estimate everyone consumes (coordinate alignment, audio mixing, call reconstruction, shared maps, robot cells). Written without naming any specific application. Establishes: the estimation scope is the flow (spec's boundary principle unchanged); four stream categories with precise semantics (per-party log without CRDT merge, jet, role-written drip, query-response) plus a summary stream with a parent-only audience; delegation eligibility rules (capability, standing, authorization coverage, completeness) and lease semantics; derived records citing input horizons (reproducibility, audit, feedback prevention as a type error); per-stream memorization with origin-vs-replica custody and discard-on-leave; nesting as the disclosure mechanism (parent reads summary only; opaque references); flow ensembles; a clock reference; the generic application-facing surface and its closed error set; a spec-style example; and a ten-item build-order gap list.
- **Status**: **Currency high, settled-ness low.** The concrete counterpart is kept outside this repo (see the entry above). The consumer-side contract this must satisfy lives in the companion project's own repo. Development discipline for keeping the two sides parallel is also kept outside this repo (see below). The spec (`rim-self-data-flows.tex`) received matching example-style additions on Sep 6, 2026: log and query-response category hints in §Streams; delegation eligibility and lease semantics in §Policies; per-stream memorization with origin/replica custody in §Memorization Policies; opaque references and "nesting as disclosure" in §Flow Mesh; a Flow Ensembles design note in §Ensembles; and a rewritten stream list plus a nesting/disclosure paragraph in §Spatial Measurements. None of it is implemented.

### `20260906_fable_markov_blanket_discipline.md` — **moved out of this repo**
- **Written**: Sep 6, 2026
- **Last modified**: Sep 6, 2026
- **What it is**: Binding development discipline for the period in which soradyne and a companion coordinate-alignment project are built side by side. Defines the Markov blanket between them as three things: two obsessively parallel contract documents in two vocabularies (each side's own contract doc, in its own repo), a single adapter package that is the only place both vocabularies and both project names appear, and a four-stratum test structure (each project alone against a fake of the other; a substrate conformance suite; end-to-end only through the adapter). Contains the vocabulary translation table (the only copy besides the adapter README), the change procedure in both directions, a list of always-wrong shortcuts and always-allowed spikes, the parity ledger format, the two-implementations rule for keeping a substrate swap possible, orchestration roles for multi-instance and hardware tests, and a definition of done. This is deliberately the only document that names both projects together — which is also why it does not belong committed in either repo.
- **Status**: **Relocated, Sep 6, 2026.** Per its own placement rule (never copy into the companion project's repo; live at the workspace root above both repos), this file was moved out of `docs/` to `~/Dropbox/Personal Projects/Nestbox/20260906_fable_markov_blanket_discipline.md` on this machine, and a copy is being placed at the workspace root on the `rim` development machine (e.g. `~/Dev/BLANKET.md`) under a name suited to that context. No longer present in this repo.

### `20260906_fable_soradyne_cascade.md`
- **Written**: Sep 6, 2026
- **Last modified**: Sep 6, 2026
- **What it is**: The soradyne-side plan that uses the estimation application's substrate requirements as the first real load and the vault's values as the second source of truth, holds every accumulated soradyne concept up against both, and issues a verdict per concept (keep / revise / retire / new) across the flow model, policies, security and privacy, boundaries and mesh, topology and transport, the app surface, and code-level concepts. Produces a consolidated model (15 concepts, 9 invariants C1–C9), then cascades it: a section-by-section restructure of the spec (new Custody, Grants and Introduction, Privacy and Disclosure, Application Surface sections; Data Geometry to an appendix; Twigs appendix retired), actions for every other design doc, an eleven-step implementation order (streams as kinds, per-stream custody, leases, references, flow ensembles, grants, clock, app surface with a Python binding, CLI), and re-derivations of the four anchoring applications: inventory with spaces as flows and items placed by owner-chosen projection; giantt with relations split by kind (REQUIRES/ANYOF/BLOCKS intrinsic on the item, SUPERCHARGES/layout/membership extrinsic on the chart) and overlay validated after materialisation; kmeep as the text/file-tree example with per-shared-note flows, public-cache attachments, and agent access as a tag-computed grant; photos retired as built and rebuilt as the first pure application of the model. Ends with a retire list for the garden pass, milestones, and what is left undecided.
- **Status**: **Currency high, settled-ness medium.** Concept verdicts and the consolidated model are proposed decisions; application sections are proposals; the retire list is a recommendation. Reverses the 20260729 granularity plan's item-is-the-flow, cached-hint disclosure, and coherence-group decisions, and resolves giantt's intrinsic/extrinsic question.

---

## Formal Models (`models/`)

### `models/README.md`
- **Updated:** 2026-09-11. Scope, retired-model rationale, commands, and requirements
  for tying future mechanism models to concrete demos.

### `models/invariants.als` — retired 2026-09-11
- **Deleted**, including Alloy runner/download wiring. Its structural assumptions no
  longer represent the design; recover historical versions from git only if needed.

### `models/Lease.tla` / `models/Lease.cfg`
- **Retained abstract sketch:** term-stamped writes under a single holder, bounded
  checks. Does not model distributed agreement, partitions, or prove implementation
  correctness. Scope clarification added; mathematical body unchanged.

### `models/check.sh`
- Runs retained TLA+ sketches only (`all` or `tla`); downloads TLC if necessary.
  Exit 0 pass, 1 failure, 2 unavailable tooling/invalid invocation. Invoked by the Rust
  `formal_models` harness; a skip is not evidence that a model passed.

---

## Reference Code

### `port_reference/giantt_core.py`
- **Written**: pre-Feb 2026 (added to repo Feb 4, 2026)
- **What it is**: Original Python implementation of Giantt's core data model. Enums (Status, Priority, RelationType, TimeConstraintType, ConsequenceType, EscalationRate), Duration handling, GianttItem, and GianttGraph. ~47 KB.
- **Status**: **Reference only, do not edit.** The Dart port in `packages/giantt_core/` is the active codebase. This file is the source-of-truth for porting fidelity — consult it when verifying that the Dart implementation matches the original Python behavior.

### `port_reference/giantt_cli.py`
- **Written**: pre-Feb 2026 (added to repo Feb 4, 2026)
- **What it is**: Original Python CLI for Giantt. Click-based command interface with file I/O, backup management, include directives. ~51 KB.
- **Status**: **Reference only, do not edit.** Same role as `giantt_core.py` — the original to port from. The Dart CLI in `packages/giantt_core/bin/giantt.dart` is the active version and has since diverged (added flow/sync support not present in the Python original).

---

## Debugging & Analysis

### `tcp-sync-analysis.md`
- **Written**: Mar 11, 2026
- **Last modified**: Mar 11, 2026
- **What it is**: Detailed debugging analysis of Giantt TCP sync between Mac and Linux, from March 2026. Identified 5 problems: remote ops not persisted, process-isolated flow instances, no subscribe/callback mechanism on DripHostedFlow, same flaw in InventoryFlow, and no incremental sync protocol. Proposed an `on_remote_op` callback fix.
- **Status**: **Mostly obsolete.** The three primary bugs identified have been fixed since this was written: (1) remote op persistence now happens directly in `FullReplicaFlow`'s background task via `append_to_journal`, (2) the separate `GianttFlow`/`InventoryFlow` wrappers no longer exist — `FullReplicaFlow` handles persistence internally, (3) horizon-based incremental sync (`HorizonExchange` + `operations_since`) is implemented. The architectural analysis of the intended layering and the "What Works" section remain accurate as historical context. The process isolation observation (Problem 2) is still structurally true but no longer a problem since persistence works.

### `20260312_giantt_sync_thoughts.txt`
- **Written**: Mar 12, 2026
- **Last modified**: Mar 12, 2026
- **What it is**: Conversation transcript from a debugging session the day after `tcp-sync-analysis.md`. Starts from the process-isolation bug (CLI writes to its own flow instance, sync process doesn't see it) and escalates into a fundamental architectural discussion. Key outcomes: (1) TCP shelved as premature — it brought unauth'd/unencrypted "normal" networking patterns; (2) the flow is the authority, not files — CLI should write to a stream, not `operations.jsonl`; (3) per-party op storage design: each flow has N files on each device (one per read/edit party), memorization and outbound queuing are separate concerns; (4) fitting is local for this flow type but not universally; (5) a 5-step plan from local persistence through simulated multi-entity testing to authenticated BLE sync. Also establishes naming discipline: this per-party-replication flow type needs its own name (became `FullReplicaFlow`), distinct from other possible flow types with different policies.
- **Status**: **Historically important, largely implemented.** Steps 1–2 of the 5-step plan are done: `FullReplicaFlow` with per-party journals (`journals/{device_id}.jsonl`), horizon-based sync, and the outbound queue was ultimately replaced by the simpler journal+horizon model (no queue needed — `operations_since(horizon)` computes what peers need on demand). The TCP-shelving decision was partially reversed in Apr 2026 when static-peer TCP was reintroduced for practical cross-machine sync, but layered properly through `EnsembleManager` rather than as raw sockets. Steps 3–4 (multi-flow, multi-schema verification) are partially covered by Docker integration tests. Step 5 (authenticated BLE sender) remains Phase 7. The architectural principles articulated here — flow as authority, apps don't touch files, per-party storage, local fitting — are the current design.

---

## Assets

### `img/soradynelogo.png`
- **What it is**: Soradyne project logo (100 KB PNG).
- **Status**: Current.
