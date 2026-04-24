# Documentation Index

Index of files in `docs/`, with approximate dates, content summaries, and obsolescence notes.

---

## rim Protocol Specification

### `rim-protocol/rim-self-data-flows.tex` (.pdf, Makefile)
- **Written**: ~Feb 2026
- **Last modified**: Feb 13, 2026
- **What it is**: The primary design document for the rim Self-Data Protocol. 1,966 lines of LaTeX defining self-data flows as the core abstraction: flows, streams (drips and jets), data geometry, flow roles, fittings, policies, security/memorization, and device topology (capsules, ensembles, parures). Includes worked examples for images, video, sound, spatial measurements, photo albums, binary data, inventory, task graphs (Giantt), and text. TikZ diagrams. Appendices comparing with Nestbox Twigs and listing implementation status.
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
