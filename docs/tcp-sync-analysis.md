# TCP Sync Architecture Analysis

Findings from debugging the Giantt TCP sync between Mac (listener) and Linux (client), March 2026.

## What Works

- **TCP transport**: Connection, handshake (UUID exchange), and framing all work correctly.
- **Initial op batch**: After the flow ID fix (commit `046b0ec`), both sides correctly send their local operations to the peer on connect. The Mac sent a 574KB payload containing ~2000 ops.
- **CRDT engine**: `ConvergentDocument` apply_local/apply_remote, materialization, and serialization are solid. The 5-primitive operation model and informed-remove semantics work as designed.
- **Flow ID matching**: After commit `046b0ec`, both sides use the workspace UUID from `.flow_id` rather than random UUIDs, so the `msg_flow_id != flow_id` filter in `DripHostedFlow::start()` now passes correctly.
- **Messenger routing**: `TopologyMessenger` correctly delivers `RoutedEnvelope` messages to broadcast subscribers. The recv loop in `add_connection` deserializes CBOR and publishes to the `incoming_tx` channel.

## The Architecture as Documented

The docs describe a clean layered system:

```
Application Wrapper (GianttFlow / InventoryFlow)
  - Owns persistence (operations.jsonl)
  - Exposes FFI surface to Dart
  - Calls flow.apply_edit() for local ops
  - Calls doc.apply_remote() for remote ops + persists
       ↕
DripHostedFlow<S> (sync orchestrator)
  - Owns the ConvergentDocument
  - Manages host assignment + failover policy
  - Broadcasts local edits via messenger
  - Listens for incoming FlowSync messages via messenger
  - Applies remote ops to document
       ↕
TopologyMessenger (routing)
  - Routes RoutedEnvelopes between peers
  - Multi-hop transparent routing
  - Flows never see BLE/TCP connections
       ↕
Transport (BLE / TCP / Simulated)
```

From `convergent_document_design.md`, the intended integration pattern is:

```rust
// Local operations are applied and broadcast
doc.apply_local(op.clone());
doc_flow.publish(op);

// Remote operations are received and merged
doc_flow.subscribe(|op| doc.apply_remote(op));
```

From `capsule-ensemble-implementation-plan.md` Phase 5.5.1:

> Refactor so the document lives _inside_ a `DripHostedFlow`, and the FFI calls write operations into the flow's "edits" jet stream rather than directly into the document.

And Phase 5.5.3 on sync protocol:

> All messages are wrapped in `RoutedEnvelope` by the messenger. [...] Flows subscribe to reachability changes and initiate horizon exchange when a new peer appears.

## The Architecture as Implemented

The actual implementation has a critical structural mismatch: `DripHostedFlow` and the application wrapper (`GianttFlow`/`InventoryFlow`) both independently interact with the `ConvergentDocument`, but `DripHostedFlow` doesn't notify the wrapper about remote operations it applies.

### Problem 1: DripHostedFlow bypasses the application wrapper for remote ops

`DripHostedFlow::start()` spawns an async listener task that processes incoming `FlowSyncMessage::OperationBatch` messages:

```rust
// drip_hosted.rs, handle_flow_sync_static()
FlowSyncMessage::OperationBatch { ops, .. } => {
    if let Ok(mut doc) = document.write() {
        for op in ops {
            doc.apply_remote(op);  // Direct to ConvergentDocument
        }
    }
}
```

This applies ops to the in-memory CRDT document but **never notifies GianttFlow**. The wrapper's `apply_remote()` method — which calls `append_op()` to persist to `operations.jsonl` — is never invoked. Result:

- Remote ops exist in memory only during the sync process lifetime
- They are not persisted to disk
- Other CLI invocations (`show`, `snapshot`, `watch`) open separate `GianttFlow` instances that load from disk, so they never see the synced state
- On process restart, all synced ops are lost

This is the **primary bug** that prevented synced items from appearing.

### Problem 2: Process-isolated flow instances

Each CLI command (`sync`, `show`, `snapshot`, `watch`) creates its own `GianttFlow` instance via `FlowRepository.loadGraph()` or `FlowClient.open()`. These are independent in-memory `DripHostedFlow` instances that each load from the same `operations.jsonl` file.

The `sync` process holds a flow with active TCP connections and a running `start()` listener. But when you run `giantt show bird`, it opens a *different* flow instance that only knows what's on disk. Even if remote ops were being applied in the sync process's memory, the `show` process wouldn't see them.

This isn't inherently wrong — process isolation is fine as long as persistence works correctly. But combined with Problem 1 (no persistence of remote ops), it means synced state is invisible to every other process.

### Problem 3: No subscribe/callback mechanism on DripHostedFlow

The docs describe a subscribe pattern:

```rust
doc_flow.subscribe(|op| doc.apply_remote(op));
```

But `DripHostedFlow` has no such mechanism. There's no way for `GianttFlow` to register a callback that fires when remote ops arrive. The `start()` method's listener task captures `Arc<RwLock<ConvergentDocument>>` directly and operates on it in isolation.

A complication: `handle_flow_sync_static` is a static method — it doesn't have access to `&self`. This is because the async task spawned by `start()` can't hold a reference to the `DripHostedFlow` instance (which lives behind a `Mutex` in the FFI layer). Instead, it captures individual `Arc` fields (`document`, `host_assignment`, `pending_edits`, etc.). Any callback mechanism would need to be an additional `Arc` field captured by the spawned task, not something accessed through `&self`.

The `messenger.incoming()` broadcast channel exists, but it's consumed exclusively by the `start()` task. The application wrapper would need to either:
1. Also subscribe to `messenger.incoming()` and duplicate the message parsing/filtering (bad: breaks encapsulation, duplicates logic)
2. Register a callback on `DripHostedFlow` that fires after remote ops are applied (good: clean separation)
3. Have `DripHostedFlow` emit applied ops on a separate channel (good: decoupled)

### Problem 4: InventoryFlow has the same structural flaw

`InventoryFlow` in `ffi/inventory_flow.rs` is structurally identical to `GianttFlow` — it wraps `DripHostedFlow<InventorySchema>` and manages persistence. It has the same `apply_remote()` method that persists ops, and the same problem: `DripHostedFlow::start()` bypasses it completely.

The Flutter demo (soradyne_demo) likely has the same underlying issue. The demo app polls `readDrip()` from the same process that runs the sync loop, so it would see in-memory state from the running `DripHostedFlow`. But whether real bidirectional sync over the messenger has been tested end-to-end in the Flutter demo is unclear — the demo may have only exercised sim accessories using a code path that doesn't involve `start()`. Regardless, persistence to disk is broken for remote ops: restarting the app would lose all state received via sync.

## Problem 5: No Incremental Sync Protocol

The docs describe a proper two-phase sync protocol in Phase 5.5.3:

```
Piece A                           Piece B
────────                          ────────
1. Exchange horizons              1. Send my horizon
   messenger.send_to(B,              messenger.send_to(A,
     FlowSync, horizon_A)              FlowSync, horizon_B)

2. Compute ops B hasn't seen      2. Compute ops A hasn't seen
   ops = doc.operations_since(       ops = doc.operations_since(
           horizon_B)                        horizon_A)

3. Send ops to B                  3. Send ops to A
4. Apply B's ops                  4. Apply A's ops
5. Both now converged             5. Both now converged
```

The `ConvergentDocument` already supports this — it has `operations_since(horizon)` and a `Horizon` type that tracks what each device has seen.

The implementation skips this entirely. On every TCP connect, both sides send **all** of their operations as a bulk `OperationBatch`. The receiving side's `doc.apply_remote()` deduplicates (it checks `(author, seq)` keys), so correctness is maintained, but the cost is O(n) on every connect rather than O(delta).

With ~2000 ops this is already 574KB per connect. As users accumulate operations over weeks/months, this will grow without bound until lazy compaction (documented but not yet implemented) kicks in. Even without compaction, horizon-based incremental sync would keep the per-connect cost proportional to what's actually new.

This isn't a correctness bug — deduplication ensures convergence — but it's a significant deviation from the documented design and will become a practical problem at scale.

## How These Map to the Self-Data Flow Concept

The core Self-Data Flow concept is that **data flows belong to the user, sync peer-to-peer, and work offline-first**. The `ConvergentDocument` + CRDT engine faithfully implements the data model. The `DripHostedFlow` correctly implements the sync orchestration semantics (host assignment, failover policies, OfflineMerge). The `TopologyMessenger` correctly abstracts routing.

But the concept breaks down at the boundary between "sync engine" and "application persistence." A Self-Data Flow should guarantee:

1. **Durability**: Once an operation is received and applied, it survives process restarts. *Currently fails*: remote ops via `DripHostedFlow::start()` are never persisted.

2. **Observability**: Any process or UI that reads the flow sees the current state, including remotely synced ops. *Currently fails*: process isolation + no persistence means synced state is invisible to other CLI commands.

3. **Separation of concerns**: The sync engine handles convergence; the application handles storage and presentation; neither needs to know the other's internals. *Currently violated*: the sync engine applies ops directly to the document, bypassing the application's persistence layer, because no notification mechanism exists between them.

4. **Efficiency**: Sync cost should be proportional to what's changed, not total history size. *Currently violated*: full op dump on every connect instead of horizon-based incremental sync.

The CRDT layer and the transport layer are each well-designed in isolation. The gap is at the seam between them — the flow layer that should mediate between "sync engine that converges state" and "application that persists and presents state" currently short-circuits the application entirely for the receive path.

## Proposed Fix: Remote Op Callback

The cleanest fix that maintains modularity:

Add an `on_remote_op` callback to `DripHostedFlow`:

```rust
pub struct DripHostedFlow<S: DocumentSchema + 'static> {
    // ... existing fields ...
    /// Optional callback invoked after each remote op is applied to the document.
    /// The application wrapper registers this to persist ops, update UI, etc.
    on_remote_op: Option<Arc<dyn Fn(&OpEnvelope) + Send + Sync>>,
}
```

Because `handle_flow_sync_static` is a static method that captures individual `Arc` fields from the struct (not `&self`), the callback must be stored as an `Arc` field and captured by the spawned task in `start()`:

```rust
// In start(), capture the callback alongside other fields:
let on_remote_op = self.on_remote_op.clone();

tokio::spawn(async move {
    loop {
        tokio::select! {
            Ok(envelope) = rx.recv() => {
                // ... existing message parsing and filtering ...
                Self::handle_flow_sync_static(
                    &document,
                    &host_assignment,
                    &pending,
                    &policy,
                    device_uuid,
                    envelope.source,
                    msg,
                    on_remote_op.as_ref(),  // pass callback through
                );
            }
        }
    }
});
```

In `handle_flow_sync_static`, after `doc.apply_remote(op)`:

```rust
FlowSyncMessage::OperationBatch { ops, .. } => {
    if let Ok(mut doc) = document.write() {
        for op in ops {
            doc.apply_remote(op.clone());
            if let Some(ref cb) = on_remote_op {
                cb(&op);
            }
        }
    }
}
```

Then `GianttFlow` and `InventoryFlow` register their `append_op` as the callback:

```rust
// In GianttFlow, after creating the DripHostedFlow:
let storage_path = self.storage_path.clone();
self.flow.set_on_remote_op(move |envelope| {
    // append to operations.jsonl
    append_op_to_disk(&storage_path, envelope);
});
```

This keeps:
- `DripHostedFlow` agnostic about storage (it just notifies)
- `GianttFlow`/`InventoryFlow` in control of persistence format
- The CRDT document as the single source of truth for in-memory state
- Clean separation between sync and storage
- The existing `handle_flow_sync_static` pattern intact (callback is just another captured `Arc`)

### Alternative: Channel-based approach

Instead of a callback, `DripHostedFlow` could expose a `remote_ops_rx: broadcast::Receiver<OpEnvelope>` channel. The application wrapper spawns a task to drain it and persist. This is more idiomatic for async Rust and avoids blocking the sync task on disk I/O, but requires the wrapper to manage an async task, which adds complexity for the FFI layer where everything is synchronous `Mutex`-based.

A hybrid approach could work: the callback enqueues ops into an `mpsc` channel, and a separate task drains the channel to disk. This decouples the sync hot path from disk writes while keeping the callback API simple for the wrapper.

## Other Issues Found During Debugging

1. **Race condition in subscribe-before-connect** (fixed in `2b37af2`): `start()` must subscribe to `messenger.incoming()` *before* `add_connection()` spawns the recv task, otherwise the peer's initial batch arrives before the subscriber exists and is silently dropped by the broadcast channel.

2. **Flow ID mismatch** (fixed in `046b0ec`): `DripHostedFlow` was constructed with `Uuid::new_v4()` as its config ID, but peers used the workspace's `.flow_id` UUID in their `FlowSyncMessage`. The `msg_flow_id != flow_id` filter in `start()` silently dropped every message.

3. **`snapshot`/`show` open independent flow instances**: CLI commands that read flow state create new `GianttFlow` instances from disk. This is architecturally fine (processes are isolated), but it means the only way for synced state to be visible to other processes is through durable persistence — which reinforces that the remote op persistence fix is the critical path item.

4. **Debug logging still present**: `drip_hosted.rs` currently has diagnostic `eprintln!` calls added during this debugging session (envelope details, flow ID matching, etc.). These should be removed or gated behind a feature flag before merging.
