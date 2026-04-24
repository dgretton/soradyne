# rim Transport Architecture

How soradyne implements networking: BLE as the universal abstraction,
multiple transport backends, the embedded engine model, and the
design constraints that follow from all of this.

This document collects architectural principles and design constraints
that span several areas — transport, process model, multiplexing,
event-driven design — into one place. For the tiered discovery/transport
plan specifically (mDNS, hole-punching, relay), see `transport_tiers.md`.

---

## BLE as the universal template

All data transfer between devices in soradyne is expressed in the
language of Bluetooth Low Energy. This does not mean BLE is always the
physical transport — it means BLE defines the abstraction boundary.

Three traits in `ble/transport.rs` define the interface:

- **`BleConnection`** — `send(&[u8])` / `recv() -> Vec<u8>` / `disconnect()`
- **`BleCentral`** — `start_scan()` / `connect(addr)` / `advertisements()`
- **`BlePeripheral`** — `start_advertising(data)` / `accept()` / `stop_advertising()`

Everything above this layer — `EnsembleManager`, `TopologyMessenger`,
`FullReplicaFlow`, application code — works exclusively with these traits.
It never knows or cares what physical transport is underneath.

**Implementations exist (or are planned) for:**

| Backend | Status | When used |
|---------|--------|-----------|
| `SimBleNetwork` | Implemented | Unit tests, in-process demos |
| Static-peer TCP | Implemented | Cross-machine sync (dev, Docker tests) |
| Real BLE (btleplug + JNI) | Implemented | macOS central, Android peripheral |
| mDNS/TCP (`MdnsBleDevice`) | Planned (Tier 2) | LAN zero-config |
| STUN + hole punch | Planned (Tier 3b) | WAN direct |
| Relay at rimm.ing | Planned (Tier 3c) | WAN fallback |
| CAN bus | Future | Automotive / embedded |
| LoRa | Future | Long-range low-power |

A device can have connections from multiple backends simultaneously.
Within a single process, one `EnsembleManager` accepts them all as
`Arc<dyn BleConnection>` — a TCP connection and a BLE connection to
different peers are handled identically. However, multiple apps on
the same device each run their own soradyne engine with their own
`EnsembleManager` (see "The embedded engine model" below). Under
BLE this is fine — the OS multiplexes. Under TCP this creates port
contention, which is an open design problem (see "TCP multiplexing").

---

## Transparent transport switching

A connection to a peer might start as BLE and switch to TCP if
soradyne discovers that the same peer is reachable over a faster
transport. The ideal behavior:

- **Best case**: The switch is invisible to the application. Data was
  flowing over BLE; now it flows over TCP. The app never notices.
- **Acceptable fallback**: soradyne closes the old connection, the app
  reconnects, the new connection happens to use TCP instead. This is
  the retry-on-disconnect path that apps should already handle.
- **Not acceptable**: The app receives TCP-specific errors
  (`ConnectionRefused`, `AddrInUse`, etc.) that would never occur
  under pure BLE. These must be caught and translated within soradyne.

The application sees only `BleConnection` — it reads, writes, and
handles disconnects. Transport selection is modular and happens below
the abstraction.

---

## Why BLE is preferred

BLE is the desired primary channel for all low-bandwidth connections
where it can be used, because:

1. **Local to the body.** BLE range is short (~10m typical). A device
   syncing over BLE is almost certainly in the physical possession of
   its owner, which is a strong security signal under rim's threat model.
2. **OS-managed multiplexing.** Multiple apps can communicate with
   the same external BLE device via GATT without coordinating. The OS
   handles connection sharing. No port conflicts, no binding races.
3. **No infrastructure.** No WiFi, no router, no internet, no accounts.
   Two devices in the same room can sync.

TCP and WAN transports extend reach when BLE is unavailable, but they
are supplements, not replacements.

---

## The embedded engine model

soradyne runs **within each app process**, linked via FFI (for Dart/Flutter)
or as a plain Rust crate (for Tauri and other native apps). There is no
background daemon that must be running for sync to work.

**Rationale**: If apps were built to depend on a running daemon, the
daemon becomes a hidden requirement — an inconsistent design pattern
that makes it unclear how to build new apps against soradyne. Instead,
each app embeds soradyne directly. The Rust code is the sync engine;
the app is the host process.

**Implications:**

- **Per-process instances.** Two apps on the same device each have their
  own soradyne engine, their own connections, their own in-memory flow
  state. This is correct — it mirrors how BLE works (each app has its
  own GATT interactions, multiplexed by the OS).

- **Lightweight embedding.** Since soradyne lives in every app's process
  memory, it should be modular. An app that syncs task lists shouldn't
  load ffmpeg. Feature-gated compilation (`--no-default-features`) and
  conditional module loading keep the footprint appropriate.

- **Short-lived vs long-lived processes.** A CLI command like `giantt add`
  runs, writes an op to the local journal, and exits. A GUI app like the
  giantt desktop app runs continuously and maintains live sync connections.
  Both are valid usage patterns. CRDT convergence ensures correctness
  regardless of how many processes write concurrently — they all converge
  when sync eventually runs.

- **On mobile**, the app IS the long-lived process. iOS/Android BLE
  background modes handle wake-ups for incoming sync.

### The `soradyne-cli` daemon

`soradyne-cli sync` is a long-running process that syncs all local flows.
It acts as a **surrogate app** — useful when no GUI app is running (e.g.,
on a headless Linux server, or for monitoring flows from a terminal).

It is **not required** for sync to work. If a desktop giantt app is
running with soradyne embedded, that app handles its own sync. Running
the CLI daemon alongside is optional and additive (e.g., to monitor
flow state in a terminal while using the app).

---

## TCP multiplexing: emulating BLE's OS-managed sharing

Under pure BLE, multiple apps on the same device connect to the same
external device independently. The OS manages the underlying radio
connection and multiplexes GATT operations. Apps don't coordinate,
don't conflict, and don't see each other.

TCP doesn't work this way. Ports are exclusive. Two processes can't
bind the same port. Connecting to a peer's listener is fine (each
connection gets its own ephemeral source port), but listening is
singular per port.

**The challenge:** To present BLE semantics to apps while using TCP
underneath, soradyne must handle the multiplexing that the OS would
handle for BLE. Each app process needs its own logical connection to
each peer, without TCP-specific errors leaking through.

**Current state:** Static-peer TCP uses one listener port per device
(port 7979). This works for the single-app case (one `soradyne-cli sync`
or one app with embedded soradyne). Multiple processes on the same
device contending for port 7979 is a known limitation.

**Future approaches (design needed):**

- A per-device-pair side-channel that is properly singular (one TCP
  listener per device pair) for negotiating how the per-app connections
  should work.
- `SO_REUSEPORT` to allow multiple processes to share the listener port,
  with message-level routing by flow ID inside `RoutedEnvelope`.
- Exclusive lock on the listener port; second process falls back to
  outbound-only connections (still useful — the listening process
  forwards via `TopologyMessenger`).

This is an open design problem. The goal is clear — TCP must be
invisible behind the BLE abstraction — but the multiplexing mechanism
is not yet decided.

---

## Event-driven, not polling

Sync should be triggered by actions, not timers:

- **Writes trigger broadcast.** When an app writes an op to a flow,
  the flow persists it locally and immediately broadcasts to connected
  peers. No polling interval.
- **Connection drops trigger reconnect.** When a peer disconnects,
  soradyne reacts to the disconnect event and attempts to reconnect.
  Exponential backoff is acceptable; a process sitting in a `sleep` loop
  as its primary mode of operation is not.
- **Peer discovery is event-based.** BLE scan results arrive as events.
  mDNS announcements arrive as events. Static peer connections are
  attempted on startup and on configuration changes, not polled.

The current implementation follows this pattern: `FullReplicaFlow::start()`
spawns a background task that listens for incoming `FlowSync` messages
via the `TopologyMessenger` subscription. Outbound sync happens
immediately on write via `runtime_handle.spawn()`. Reconnection on
disconnect uses the both-sides-connect pattern (each side listens AND
connects; whichever starts second finds the other's listener).

---

## Discovery progression

How devices find each other, from simplest to most robust:

1. **Now: Static peer config.** IP:port pairs recorded in
   `~/.soradyne/static_peers.json`, typically Tailscale virtual IPs.
   Works, but requires manual configuration and a VPN.

2. **Tier 2: mDNS/DNS-SD.** Zero-configuration LAN discovery. Devices
   advertise `_rim._tcp.local` with encrypted advertisement data in
   TXT records. No static IPs, no servers, no VPN. See `transport_tiers.md`.

3. **Tier 3b: STUN + rendezvous + hole punching.** WAN connectivity
   without infrastructure. A lightweight rendezvous endpoint (possibly
   at rimm.ing, a domain owned by the project) handles presence; STUN
   provides public address discovery; UDP hole punching establishes
   direct connections. Modeled after Signal's approach. See `transport_tiers.md`.

4. **Tier 3c: Relay fallback.** For symmetric NATs and corporate firewalls
   where hole punching fails. A relay server at rimm.ing forwards
   Noise-encrypted traffic. Zero trust — the relay sees only ciphertext.

All tiers produce `Box<dyn BleConnection>`. The code above the transport
layer doesn't change.

---

## Security invariants

These hold regardless of transport tier:

- **All connections are authenticated.** Mutual authentication via Ed25519
  keys exchanged during pairing. Non-capsule-members cannot participate.
- **All traffic is encrypted.** Noise IKpsk2 session encryption, keyed
  from capsule PSK + device keys. The transport (BLE, TCP, relay) sees
  only ciphertext.
- **Apps don't see capsules.** Capsule membership and key management
  are internal to soradyne. Apps work with flows. The FFI surface
  exposes flow operations, not capsule operations. (Setup/pairing is
  the exception — mobile apps may need to present a pairing UI, but
  this is ideally one-time.)

---

## Relationship to existing code

| Concept | Where in code |
|---------|--------------|
| BLE traits | `ble/transport.rs` |
| SimBLE (tests) | `ble/simulated.rs` |
| Real BLE central | `ble/btleplug_central.rs` (macOS/Linux, feature-gated) |
| Real BLE peripheral | `ble/android_peripheral.rs` (Android, JNI) |
| Static-peer TCP | `topology/manager.rs` (`EnsembleManager::start()`) |
| Transport switching | Not yet implemented |
| Per-process embedding | `ffi/pairing_bridge.rs` (global singleton per process) |
| Event-driven sync | `flow/types/drip_hosted.rs` (`FullReplicaFlow::start()`) |
| CLI daemon | `soradyne_cli/src/main.rs` (`handle_sync()`) |
| Multiplexing | Open design problem |
