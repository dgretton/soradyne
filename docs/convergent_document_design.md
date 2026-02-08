# Convergent Document Design

Design decisions for Soradyne's distributed document system, enabling leaderless, peer-to-peer synchronization of structured data (Giantt graphs, photo albums, network topology, etc.).

## Goals

1. **Leaderless Convergence**: Any device can make edits offline; all devices converge to the same state when synced
2. **Informed-Remove Semantics**: Removes only affect states the remover had observed (prevents "deletion disasters")
3. **Causal Awareness**: Track what each device had seen when making decisions
4. **Preserve-Over-Lose**: When conflicts occur, prefer preserving information over losing it
5. **History Visibility**: Keep operation history available for debugging and consistency checking

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    ConvergentDocument<S>                        │
│  (Generic over Schema S that defines items, fields, and sets)   │
├─────────────────────────────────────────────────────────────────┤
│  Operations (5 Primitives)                                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐               │
│  │  AddItem    │ │ RemoveItem  │ │  SetField   │               │
│  └─────────────┘ └─────────────┘ └─────────────┘               │
│  ┌─────────────┐ ┌─────────────┐                               │
│  │  AddToSet   │ │RemoveFromSet│                               │
│  └─────────────┘ └─────────────┘                               │
├─────────────────────────────────────────────────────────────────┤
│  Causal Context                                                 │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Horizon: Map<DeviceId, SeqNum>                  │           │
│  │ - Tracks what each device has seen              │           │
│  │ - Enables informed-remove semantics             │           │
│  └─────────────────────────────────────────────────┘           │
├─────────────────────────────────────────────────────────────────┤
│  State Materialization                                          │
│  ┌─────────────────────────────────────────────────┐           │
│  │ Materializer: Operations → Current State        │           │
│  │ - Latest-wins for scalar fields                 │           │
│  │ - Additive merge for collections                │           │
│  │ - Existence = has AddItem without later Remove  │           │
│  └─────────────────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────┘
```

## The Five Primitive Operations

All document mutations express through five operations:

### 1. AddItem
Creates a new item in the document with a unique ID.

```rust
AddItem { item_id: ItemId, item_type: String }
```

If the same item_id appears from multiple devices, they merge (same item created concurrently).

### 2. RemoveItem
Marks an item as removed. Uses informed-remove semantics.

```rust
RemoveItem { item_id: ItemId, horizon: Horizon }
```

The `horizon` records what operations the remover had seen. The item only disappears for operations that were concurrent with or after this remove. Operations from other devices *before* they learned of the remove are preserved.

### 3. SetField
Sets a scalar property on an item.

```rust
SetField { item_id: ItemId, field: String, value: Value }
```

Multiple concurrent SetField operations on the same field resolve by timestamp, with device ID as tiebreaker.

### 4. AddToSet
Adds an element to a collection property (tags, relations, charts).

```rust
AddToSet { item_id: ItemId, set_name: String, element: Value }
```

Element becomes visible. Multiple adds of the same element are idempotent.

### 5. RemoveFromSet
Removes an element from a collection property. Uses informed-remove.

```rust
RemoveFromSet { item_id: ItemId, set_name: String, element: Value, observed_add_ids: Vec<OpId> }
```

Only removes the specific add operations the remover had observed. Concurrent adds on other devices remain visible.

## Operation Composition Examples

### Giantt: Add a task with dependencies

```rust
// Add new task
AddItem { item_id: "task_123", item_type: "GianttItem" }

// Set scalar properties
SetField { item_id: "task_123", field: "title", value: "Review PR" }
SetField { item_id: "task_123", field: "status", value: "TODO" }
SetField { item_id: "task_123", field: "priority", value: "HIGH" }
SetField { item_id: "task_123", field: "duration", value: "2h" }

// Add to collection properties
AddToSet { item_id: "task_123", set_name: "tags", element: "code-review" }
AddToSet { item_id: "task_123", set_name: "charts", element: "Sprint 5" }
AddToSet { item_id: "task_123", set_name: "requires", element: "task_456" }
```

### Photo Album: Add photo with edits

```rust
AddItem { item_id: "photo_abc", item_type: "Photo" }
SetField { item_id: "photo_abc", field: "block_id", value: <content-addressed-hash> }
SetField { item_id: "photo_abc", field: "crop", value: { left: 0.1, top: 0.2, ... } }
AddToSet { item_id: "photo_abc", set_name: "tags", element: "vacation" }
AddToSet { item_id: "photo_abc", set_name: "faces", element: "person_xyz" }
```

### Network Topology: Register device connection

```rust
AddItem { item_id: "device_A", item_type: "NetworkNode" }
SetField { item_id: "device_A", field: "last_seen", value: timestamp }
AddToSet { item_id: "device_A", set_name: "capabilities", element: "storage" }
AddToSet { item_id: "device_A", set_name: "connected_to", element: "device_B" }
```

## Informed-Remove: Preventing Deletion Disasters

The core problem: Device A deletes an item while Device B (offline) is actively editing it. When they sync, whose intent wins?

### Solution: Informed-Remove Semantics

1. Every operation carries the author's `Horizon` showing what they had seen
2. A `RemoveItem` only "defeats" operations it had observed when issued
3. Operations concurrent with the remove (on other devices) survive

**Example scenario:**
```
Device A (online):     [edit1] [edit2] [delete]
Device B (offline):    [edit1] ......... [edit3] [edit4]
                                ↑ B goes offline here

When they sync:
- delete's horizon includes edit1, edit2 but NOT edit3, edit4
- Result: Item exists with edit3 and edit4 applied
- Device A's delete only removed the state they knew about
```

This means:
- You can't delete someone else's work you haven't seen
- Uninformed deletes don't destroy important edits
- If you want something truly gone, sync first, then delete

## Doctor vs Sync Checker

Two separate tools serve different purposes:

### Doctor (Local CLI Tool)
- Single-user, single-device health check
- Validates graph structure (cycles, missing dependencies)
- Runs against materialized state
- Used during normal Giantt CLI operations
- Fast, synchronous

### Sync Checker (Multi-Device Analysis)
- Multi-device consistency analysis
- Detects divergence, stale devices, merge conflicts
- Analyzes operation logs across devices
- Used during sync or on-demand
- May involve network communication

The Doctor doesn't need to know about convergence; the Sync Checker doesn't need to validate graph semantics. They serve different layers.

## Intent Signaling (Future Layer)

Meta-communication for reducing conflicts:

```rust
// "I'm actively editing this item"
Intent::Editing { item_id, device_id, started_at }

// "I'm about to delete these items"
Intent::PendingDelete { item_ids, device_id }

// "I'm reorganizing this section"
Intent::Reorganizing { scope, device_id }
```

Intents are advisory, not authoritative. They help UIs show "Alice is editing..." and encourage users to coordinate, but don't block operations.

## Lazy Hash-Based Compaction

As operation logs grow, we need garbage collection. The principle: compact when all devices agree on the current state.

### Mechanism

1. Each device computes `state_hash = hash(materialize(all_ops))`
2. Devices exchange state hashes during sync
3. When all known devices report the same hash:
   - Operations can be compacted to a snapshot
   - Old operations can be pruned
4. Any device rejoining must receive the snapshot

### Properties

- **Safe**: Only compact when everyone agrees
- **Lazy**: Don't force compaction; let it happen naturally
- **Resumable**: Devices that were offline get snapshots
- **Verifiable**: Hash proves state equivalence

## Schema Definition

A schema defines the structure of items in a convergent document:

```rust
trait DocumentSchema {
    /// Item types this schema supports
    type ItemType: ItemTypeSpec;

    /// Materialize state from operations
    fn materialize(ops: &[Operation]) -> DocumentState;

    /// Schema-specific validation
    fn validate(state: &DocumentState) -> Vec<ValidationIssue>;
}
```

Schemas bridge the generic convergence machinery and domain-specific logic (Giantt items, photos, network nodes).

## Integration with SelfDataFlow

`ConvergentDocument<S>` integrates with Soradyne's `SelfDataFlow` abstraction:

```rust
// Operations flow through SelfDataFlow for sync
let doc_flow: SelfDataFlow<DocumentOp<GianttSchema>> = ...;

// Local operations are applied and broadcast
doc.apply_local(op.clone());
doc_flow.publish(op);

// Remote operations are received and merged
doc_flow.subscribe(|op| doc.apply_remote(op));
```

This enables:
- P2P sync without central server
- Offline-first operation
- Automatic convergence when devices reconnect

## Implementation Phases

1. **Phase 1**: Generic `ConvergentDocument<S>` with the 5 operations
2. **Phase 2**: `GianttSchema` definition with Giantt-specific semantics
3. **Phase 3**: Integration with existing Dart CLI (read/write through operations)
4. **Phase 4**: SelfDataFlow integration for P2P sync
5. **Phase 5**: Sync Checker implementation
6. **Phase 6**: Intent signaling layer
7. **Phase 7**: Lazy compaction
