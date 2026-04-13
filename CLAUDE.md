# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Soradyne is a proof-of-concept protocol for secure, peer-to-peer Self-Data Flows with CRDT-based synchronization. The monorepo also contains Giantt, a task dependency management system being ported from Python to Dart to use Soradyne as its sync backend.

**Guiding constraint:** "soradyne apps don't touch the radio." All BLE is owned by Rust. Flutter is a UI front-end only. The upcoming Tauri app will use `soradyne_core` as a plain Rust crate with no FFI overhead.

**Reference materials** (do not edit):
- `giantt-original/` - Symlink to original Python CLI (external, for reference only)
- `docs/port_reference/` - Python source files being ported (`giantt_core.py`, `giantt_cli.py`)
- `giantt-design-notes/` - Feature specs, notation docs, CLI manual

## Brand & Naming Conventions

**rim** is the project's brand name and must always appear all-lowercase in public-facing contexts. Two rules apply:

1. **Documentation and public API surfaces**: Always `rim` — never `Rim` or `RIM`. This covers markdown docs, Rust doc comments (`///`, `//!`), README files, BLE service/characteristic names, and any interface exposed to other developers.

2. **Internal code identifiers**: Follow language conventions (PascalCase for types, SCREAMING_SNAKE_CASE for constants). However, **prefer namespacing over concatenation** where possible — e.g., `rim::Protocol` rather than `RimProtocol` — so the brand name stays lowercase in the identifier itself. Language-forced capitalization deep inside internal identifiers is acceptable; public API surfaces are not.

## Monorepo Structure

```
apps/
  giantt/              # Flutter app for Giantt (task management UI)
  inventory/           # Flutter app for personal inventory management
  soradyne_demo/       # Flutter demo app (pairing + flow demo + albums)
packages/
  giantt_core/         # Dart: Core giantt logic (parser, graph, storage, validation)
  soradyne_core/       # Rust: Core Soradyne library
  soradyne_flutter/    # Flutter plugin wrapping soradyne_core via FFI
  ai_chat_flutter/     # Flutter package for AI chat with action calling
```

## Build Commands

### Rust (soradyne_core)

```bash
cd packages/soradyne_core

# Standard build (default features include video-thumbnails/ffmpeg)
cargo build --release

# macOS with BLE central (btleplug) — required for soradyne_demo on macOS
cargo build --release --features ble-central --no-default-features

# After macOS build, copy dylib to the Flutter plugin:
cp target/release/libsoradyne.dylib ../../packages/soradyne_flutter/macos/libsoradyne.dylib

# Run the single integration test
cargo test --test three_piece_capsule --no-default-features

# Run unit tests
cargo test --no-default-features

# Run examples
cargo run --example block_storage_demo
cargo run --example heartrate_demo
```

### Android Cross-Compilation

```bash
# Prerequisites (one-time):
rustup target add aarch64-linux-android
cargo install cargo-ndk
# Install Android NDK via Android Studio SDK Manager

cd packages/soradyne_core
cargo ndk -t arm64-v8a build --release --no-default-features

mkdir -p ../soradyne_flutter/android/src/main/jniLibs/arm64-v8a
cp target/aarch64-linux-android/release/libsoradyne.so \
   ../soradyne_flutter/android/src/main/jniLibs/arm64-v8a/
```

Both `.so` and `.dylib` are gitignored and must be rebuilt locally after a fresh clone.

### Flutter/Dart

```bash
# Bootstrap all packages (run from repo root)
melos bootstrap

# Run tests
melos run test:flutter
cd packages/giantt_core && dart test          # single package
cd apps/giantt && flutter test

# Run analysis
melos run analyze

# Run demo app
cd apps/soradyne_demo/flutter_app && flutter run -d macos
cd apps/soradyne_demo/flutter_app && flutter run -d <android-device-id>

# Run Giantt CLI
dart run giantt_core:giantt

# Install/update the system-wide `giantt` command (compiled native binary at /usr/local/bin/giantt)
# Note: `dart pub global activate` won't work here due to spaces in the Dropbox path — compile directly instead.
cd packages/giantt_core && dart compile exe bin/giantt.dart -o /usr/local/bin/giantt
```

## Architecture: soradyne_core (Rust)

### Layer overview (bottom to top)

```
convergent/   — CRDT engine: Operation, ConvergentDocument, schemas
    ↓
flow/         — DripHostedFlow: CRDT-backed sync with host failover
    ↓
ble/          — Transport layer: traits + simulated + btleplug + android JNI
    ↓
topology/     — Pairing (one-time) + EnsembleManager (ongoing)
    ↓
ffi/          — C ABI surface exposed to Flutter (Dart FFI)
```

### `ble/` — Transport Abstraction

Three traits in `ble/transport.rs`:
- **`BleConnection`** — `send(&[u8])` / `recv() -> Vec<u8>` / `disconnect()` — one peer link
- **`BleCentral`** — `start_scan()` / `connect(addr) -> BleConnection` / `advertisements()` receiver
- **`BlePeripheral`** — `start_advertising(data)` / `accept() -> BleConnection` / `stop_advertising()`

Four implementations:
- `simulated.rs` — `SimBleNetwork`: in-process channels for tests and local demos
- `btleplug_central.rs` — `BtleplugCentral` (macOS/Linux/Windows BLE central via CoreBluetooth). Compiled only with `ble-central` feature. Handles length-prefixed framing (4-byte LE length header + 500-byte chunks) because Android caps GATT values at 512 bytes.
- `android_peripheral.rs` — `AndroidBlePeripheral` (Android BLE peripheral via JNI into `BluetoothLeAdvertiser` + `BluetoothGattServer`). Compiled only on `target_os = "android"`. Uses the same framing as btleplug central, flow-controlled by `onNotificationSent`.
- **Platform asymmetry**: btleplug 0.11 is Central-role only (no advertising API). Therefore Mac is always the *joiner* (central) and Android is always the *inviter* (peripheral) for real BLE pairing.

GATT layout (`gatt.rs`):
- `soradyne_service_uuid()` — service UUID derived from `("soradyne.rim", "service")` via SHA-256
- `envelope_char_uuid()` — characteristic for all protocol messages (`WRITE_WITHOUT_RESPONSE | NOTIFY`)
- `MessageType` enum: `TopologySync`, `FlowSync`, `CapsuleGossip`
- `RoutedEnvelope` — `{source, destination, ttl, message_type, payload}`, CBOR-serialized

Java callback bridge (`SoradyneGattCallback.java`): `nativeOnConnected` is called from `onDescriptorWriteRequest` when CCCD=0x01 (not from `onConnectionStateChange`) to ensure the central has subscribed before the first notification is sent.

### `topology/` — Pairing and Ensemble

**Two separate BLE lifecycles:**

1. **Pairing** (`pairing.rs`, `PairingEngine`): one-time capsule establishment
   - Inviter advertises `PAIRING_ADV_MARKER`; joiner scans and connects
   - X25519 ECDH key exchange → 6-digit PIN (SHA-256 of shared secret)
   - On PIN confirm: encrypted `CapsuleKeyBundle` transferred (AES-256-GCM)
   - Both sides persist the capsule; BLE connection then drops
   - FFI: `soradyne_pairing_start_invite` / `soradyne_pairing_start_join` / `soradyne_pairing_confirm_pin` / `soradyne_pairing_submit_pin`

2. **Ensemble** (`manager.rs`, `EnsembleManager`): ongoing post-pairing sync
   - Advertises/scans using **encrypted capsule-hinted advertisements** (not raw service UUID)
   - `EnsembleManager::start(central, peripheral)` runs the full loop: advertise + scan + connect + accept + topology sync
   - `TopologyMessenger` (`messenger.rs`) routes `RoutedEnvelope` messages via connected peers; multi-hop via TTL
   - Capsule topology graph (`ensemble.rs`): `PiecePresence`, `TopologyEdge`, `ConnectionQuality`
   - **Not yet started with real BLE** (Phase 7): currently `bridge_get_ensemble` creates the manager but `start()` is not called

### `convergent/` — CRDT Engine

Five primitive operations (serde enum, externally tagged):
```
{"AddItem":      {"item_id": "…", "item_type": "InventoryItem"}}
{"RemoveItem":   {"item_id": "…"}}
{"SetField":     {"item_id": "…", "field": "…", "value": <untagged Value>}}
{"AddToSet":     {"item_id": "…", "set_name": "…", "element": <Value>}}
{"RemoveFromSet":{"item_id": "…", "set_name": "…", "element": <Value>, "observed_add_ids": […]}}
```

`Value` uses `#[serde(untagged)]`, so `Value::String("x")` serializes to `"x"`, `Value::Int(42)` to `42`, etc.

Two schemas: `giantt.rs` (GianttSchema) and `inventory.rs` (InventorySchema with `InventoryItem`: id, category, description, location, tags).

### `flow/types/drip_hosted.rs` — DripHostedFlow

CRDT-backed sync where one piece is the authoritative host:
- Host selection via configurable policy (FirstEligible, BestConnected, Preferred, Scored)
- Under `OfflineMerge` failover, all pieces accept edits locally and converge via CRDT on reconnect
- Wire protocol: CBOR-serialized `FlowSync` messages in `RoutedEnvelope`
- `set_ensemble(messenger, topology)` wires the flow to a topology's messenger for routing

### `ffi/` — C ABI Surface

**`pairing_bridge.rs`** — Global `PAIRING_BRIDGE: RwLock<Option<PairingBridge>>` singleton holding:
- `identity: DeviceIdentity`
- `capsule_store: Arc<TokioMutex<CapsuleStore>>`
- `sim_network: Arc<SimBleNetwork>` (always present for sim accessories / local tests)
- `runtime: tokio::runtime::Runtime`
- `ensemble_managers: Arc<TokioMutex<HashMap<Uuid, Arc<EnsembleManager>>>>`

Key functions: `soradyne_pairing_init` / `soradyne_pairing_create_capsule` / `soradyne_pairing_list_capsules` / `soradyne_pairing_start_invite` / `soradyne_pairing_start_join` / `soradyne_pairing_confirm_pin` / `soradyne_pairing_submit_pin` / `soradyne_pairing_get_state` / `soradyne_pairing_add_sim_accessory` / `soradyne_pairing_get_device_id` / `soradyne_ble_debug`

`bridge_get_ensemble(capsule_id)` is a crate-internal helper used by `inventory_flow.rs` to lazily create and retrieve the `EnsembleManager` for a capsule.

**`inventory_flow.rs`** — Global `INVENTORY_REGISTRY` holding open `InventoryFlow` handles (keyed by UUID string). The flow UUID is typically the capsule UUID. `soradyne_inventory_connect_ensemble` calls `bridge_get_ensemble` to wire the flow to the capsule's topology messenger.

**JNI callbacks** (Android only): `Java_com_soradyne_flutter_SoradyneGattCallback_nativeOn{Connected,Write,Disconnected,NotificationSent}` — called from Binder threads, so must use `std::sync::Mutex` (not `tokio::sync::Mutex`) and `try_send` instead of `.await`.

**`ANDROID_CONTEXT`** (`OnceLock<GlobalRef>`): set by `SoradyneFlutterPlugin.kt` via `nativeSetContext()` when the plugin attaches, before `soradyne_pairing_init` is called. Required to create `BluetoothLeAdvertiser`.

## Architecture: soradyne_demo Flutter App

```
main.dart                    — MultiProvider(AlbumService, PairingService)
screens/
  activity_selector_screen   — home; routes to each activity
  capsule_list_screen        — list capsules; open detail; invite/join
  pairing_screen             — state-driven: idle→PIN→transferring→complete
  flow_demo_screen           — shared notes via InventoryFlow CRDT
  album_*                    — photo album browsing
services/
  pairing_service.dart       — ChangeNotifier; wraps PairingBindings; exposes
                               .bindings (PairingBindings), .deviceId, .capsules
ffi/
  pairing_bindings.dart      — typed Dart FFI for all soradyne_pairing_* symbols;
                               exposes .lib (DynamicLibrary) for reuse
  inventory_bindings.dart    — typed Dart FFI for all soradyne_inventory_* symbols;
                               accepts DynamicLibrary from PairingBindings
```

`PairingService` polls `soradyne_pairing_get_state` every 500 ms during an active pairing session via a Timer, driving the `PairingScreen` state machine. `FlowDemoScreen` manages its own `InventoryFlow` handle and polls `soradyne_inventory_read_drip` every 2 seconds.

## Architecture: Giantt Core (Dart)

- `models/` — GianttItem, Relation, TimeConstraint, Duration, Status, Priority
- `parser/` — Parses `.giantt` text files into item graphs
- `graph/` — Dependency graph with cycle detection (inline in `giantt_graph.dart`)
- `storage/` — FileRepository, DualFileManager (include/occlude system), atomic writes, backups
- `validation/` — GraphDoctor for finding issues in graphs
- `logging/` — Log entry tracking with occlusion support
- `commands/` — CLI commands extending `CliCommand<T>` base class

Key design notes:
- Items support multiple `timeConstraints` (List), not singular
- Include/occlude system: active items in `include/`, archived in `occlude/`
- Relations are bidirectional (auto-created in graph operations)

## Key Concepts

- **SelfDataFlow** — user-owned data stream that syncs peer-to-peer. Generic over `T: Send + Sync + Clone`.
- **Capsule** — a named trust group of pieces (devices). Holds a `CapsuleKeyBundle` for encryption. Established via `PairingEngine`.
- **Piece** — one device's slot in a capsule, with a role (Full, Accessory) and device_id.
- **DripHostedFlow** — CRDT-backed hosted flow; one piece acts as host; others send diffs; host broadcasts merged state.
- **SimBleNetwork** — in-process BLE transport for tests. Always present in the bridge; used for sim accessories.
- **Data Dissolution / Crystallization** — erasure-coding data across devices (reed-solomon) for security/resilience.

## Phase History (for context)

- **Phase 4**: PairingEngine + FFI bridge + Flutter pairing UI
- **Phase 5**: DripHostedFlow with host assignment + failover
- **Phase 5.5**: FFI bridge + Flutter UI for DripHostedFlow
- **Phase 6.1**: Three-piece capsule integration tests (all simulated)
- **Phase 6.2**: Mixed real+simulated BLE — Android peripheral (JNI) + Mac central (btleplug) — **real BLE pairing working**
- **Phase 6.3**: Flow Demo — shared notes via inventory CRDT, wired to capsule ensemble
- **Phase 7 (planned)**: Post-pairing persistent BLE — `EnsembleManager::start()` with real transport so DripHostedFlow syncs CRDT ops across devices over BLE
