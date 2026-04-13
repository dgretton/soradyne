# rim Authorization Model

Two separate authorization domains govern access to flows. They operate
at different layers and solve different problems.

---

## 1. Intra-capsule: one person, many devices

A capsule represents one person's collection of devices. All pieces in a
capsule are equivalent gateways to the same experience. There is no
per-flow or per-stream gating within a capsule: when a new device joins,
it immediately has the same access as every other piece.

This means intra-capsule authorization is a **device management** problem,
not a data authorization problem. The interesting questions are:

- How does a new piece prove it should be in the capsule? (Pairing: ECDH
  + PIN confirmation, physical proximity.)
- How is a compromised piece handled? (Retire the whole capsule, rebuild.)

Applications never see capsules. From an app's perspective, it
authenticates to rim (via an embedded auth surface, analogous to Link for
payments) and then has access to its flows. The capsule machinery is
entirely beneath the app's API boundary.

## 2. Inter-capsule: many people, shared flows

When different people (different capsules) share a flow -- an 8-person
photo album, a collaborative task graph -- authorization is per-stream
and must be cryptographically enforced end-to-end.

Key properties:

- **Per-stream granularity**: not all participants need the same access.
  Everyone might read the photos, but only the album creator modifies
  the participant list.
- **Multi-party**: not limited to two-sided sharing. N capsules can be
  authorized for the same flow.
- **E2E encrypted**: key material is scoped to the stream level and
  distributed to authorized capsules. An unauthorized capsule cannot
  read the stream even if it can observe the network traffic.
- **Invitation/acceptance**: sharing a flow with another capsule requires
  an explicit authorization step (analogous to pairing but at a higher
  level -- "I'm granting your capsule read access to this stream").
- **Revocable**: access can be withdrawn, requiring key rotation for
  affected streams.

This domain is **not yet designed or implemented**. It is the primary
open protocol design problem for multi-user rim.

## 3. App integration: the "Link model"

Applications that use rim should not need users to "install the rim app
first." Instead, rim provides an embeddable auth surface (webview, system
sheet, or platform-appropriate equivalent) that appears within the app's
own UI flow when needed. The analogy is Link (the payment service):

1. User is in an app (e.g., Giantt) and wants to sync data.
2. The app calls a rim SDK function that presents a rim-branded auth
   surface, styled to the context but clearly a separate trust domain.
3. User authenticates to their rim identity (biometric, PIN, SMS --
   whatever the platform supports). They see "your devices," not
   "capsules."
4. The app receives an opaque session/handle. It can now create and
   access flows by UUID.
5. The app never learns what a capsule is, never names one, never
   manages devices.

A dedicated rim app/settings screen can exist for device management
(add/remove devices, see connection status, retire a capsule), but it is
not the entry point. The entry point is always in-context, inside
whatever app needs rim.

## 4. What apps see vs. what soradyne sees

| Layer | Sees | Does not see |
|---|---|---|
| Application (Giantt, etc.) | Flow UUIDs, stream handles, `soradyne_session_start()`, `soradyne_flow_open(id, type)` | Capsules, pieces, ensembles, key bundles, topology |
| rim auth surface | "Your devices," add/remove device UX, identity confirmation | Flow contents, app semantics |
| soradyne_core | Capsules, pieces, ensembles, key bundles, transport, topology, CRDT engine | App-level UI, user-facing terminology |

## 5. Naming discipline

- **capsule**: internal soradyne term for a trust group of devices owned
  by one person. Never appears in app code or user-facing UI.
- **piece**: internal soradyne term for a device's identity within a
  capsule. Never appears in app code or user-facing UI.
- **flow**: the unit of synchronized data. Apps know flow UUIDs and
  types. This is the primary abstraction apps interact with.
- **multichart** (Giantt-specific): a flow containing one or more Giantt
  charts. The user-facing term might be "chart group" or similar; the
  internal noun is multichart.

## 6. Implications for current code

The existing FFI surface exposes `soradyne_flow_connect_ensemble(handle,
capsule_id)`, which requires the app to know a capsule UUID. This is a
layer violation. The target API:

```
soradyne_session_start()           // rim auth; returns opaque session
soradyne_flow_open(flow_id, type)  // open by UUID, get handle
soradyne_flow_sync(flow_id)        // enable sync; runtime handles the rest
```

Under the hood, `soradyne_flow_sync` looks up which capsule(s) this
device belongs to and starts syncing the flow with capsule peers. The
app never provides a capsule ID.
