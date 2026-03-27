<div align="center">
  <img src="docs/img/soradynelogo.png" alt="soradyne Logo" width="400">

  *the sync engine behind [rim](https://instagram.com/reclaim.intimate.mutuality)*
</div>

# soradyne

A protocol (under development) for secure, peer-to-peer self-data flows. soradyne embodies real ownership of your data — split across physical devices you hold, synced over Bluetooth, encrypted end-to-end, with no cloud in between.

soradyne is the technical core of **[rim](https://www.rim.gs/)** — a project reclaiming data sovereignty in the age of wearables. rim envisions person-to-person systems for storing and sharing intimate live data streams, using SD-core wearables and edge-based storage instead of the cloud. soradyne is the protocol that is going to make those devices work together.

## How It Works

### Data Dissolution and Crystallization

Your data doesn't live on one device because it could be lost. soradyne **dissolves** it — splitting files across multiple physical storage devices using Shamir secret sharing and Reed-Solomon erasure coding. Each device holds an encrypted shard. No single shard reveals anything. Any sufficient subset of your devices can **crystallize** the data back.

```
Your file
  ↓
AES-256-GCM encryption (unique key per block)
  ↓
Reed-Solomon erasure coding → n shards (e.g. 5)
  ↓
Shamir secret sharing of the encryption key → n key shares
  ↓
(shard + key share) distributed to each rimsd device
  ↓
Any k-of-n devices (e.g. 3-of-5) → full reconstruction
```

You can initialize your devices via CLI, dissolve data across them, and crystallize it back from any threshold subset, even if some devices are lost or damaged. Right now, crystallization and dissolution work across any combination of local bulk storage media like flash drives and SD cards, and file system directories.

### Self-Data Flows and CRDT Sync

soradyne's flow code lets multiple devices collaborate on shared data structures that converge automatically. Abstractly, we call the channels we move data over **streams**, and a stream that constitutes an eventually-consistent synthesis of multiple other streams is called a **drip**. The **convergent document** system is a drip that uses a CRDT to materialize lines of text edited by multiple secure devices. (We don't want to get stuck in the world of text forever like the rest of the internet, but it's a place to start!) The CRDT has five primitive operations (add, remove, set field, add to set, remove from set) and causal tracking that guarantees all devices reach the same state, regardless of edit order.

When two people edit the same data on different devices:

```
Device A writes op₁    Device B writes op₂ (concurrently)
        ↓                       ↓
   local journal           local journal
        ↓                       ↓
        ←── sync over BLE ──→
        ↓                       ↓
   materialize()           materialize()
        ↓                       ↓
   same result             same result
```

**Current state:** Three-device sync has been integration-tested end-to-end with topology routing across a "mesh" (a 3-node line).

### Bluetooth Transport

soradyne manages BLE; only read/write/status etc. are exposed at the application level. The BLE layer handles device discovery, pairing, and encrypted communication:

- **Pairing**: X25519 key exchange over BLE, confirmed with a 6-digit PIN derived from the shared secret, followed by encrypted transfer of capsule credentials (AES-256-GCM)
- **Sessions**: Noise IKpsk2 protocol (the same framework used by Signal and WireGuard) with pre-shared keys bound to capsule membership
- **Topology**: Devices form mesh networks and route messages with TTL-based forwarding — no hub required

BLE pairing works between Android (peripheral) and macOS (central) at the moment. Also, any device with an internet connection can still participate using a TCP-backed bluetooth mimic. Multi-hop mesh sync over BLE is under development.

## Security

soradyne uses industry-standard cryptography throughout:

| Layer | Primitive | Implementation |
|-------|-----------|----------------|
| Key exchange | X25519 ECDH | `x25519-dalek` |
| Encryption | AES-256-GCM | `aes-gcm` (AEAD) |
| Signing | Ed25519 | `ed25519-dalek` |
| Key derivation | HKDF-SHA256 | `hkdf` |
| Session encryption | Noise IKpsk2_25519_AESGCM_SHA256 | `snow` |
| Erasure coding | Reed-Solomon | `reed-solomon-erasure` |
| Memory safety | Zeroize on drop | `zeroize` |

We have rolled our own crypto (bad!) for Shamir secret sharing over GF(256) but intend to either incorporate an existing crate or contribute one to be vetted by others.

Every encryption operation uses a fresh random nonce. Per-block master keys are never reused. Capsule key bundles support epoch-based rotation. The protocol runs without any hub-spoke concept or cloud dependency.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Flutter / CLI                                   │
│  (UI and interaction layer)                      │
├─────────────────────────────────────────────────┤
│  FFI bridge (C ABI)                              │
├─────────────────────────────────────────────────┤
│  soradyne_core (Rust)                            │
│                                                  │
│  storage/     dissolution, erasure coding,       │
│               block management, rimsd devices    │
│                                                  │
│  convergent/  CRDT engine, schemas,              │
│               causal horizon tracking            │
│                                                  │
│  flow/        DripHostedFlow, host election,     │
│               journal persistence, sync          │
│                                                  │
│  ble/         transport traits, BLE central      │
│               (btleplug), BLE peripheral (JNI),  │
│               simulated network, Noise sessions  │
│                                                  │
│  topology/    pairing engine, ensemble manager,  │
│               mesh routing                       │
│                                                  │
│  identity/    device keys, capsule key bundles,  │
│               X25519/Ed25519, HKDF               │
└─────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust (latest stable)
- Flutter (for demo apps)
- Android NDK (for Android builds)

### Build and Run

```bash
# Build the core library
cd packages/soradyne_core
cargo build --release

# Run the dissolution storage demo
cargo run --example block_storage_demo

# Run tests
cargo test --no-default-features

# Build with BLE central support (macOS)
cargo build --release --features ble-central --no-default-features

# Bootstrap Flutter packages
melos bootstrap

# Run the demo app
cd apps/soradyne_demo/flutter_app && flutter run -d macos
```

### Initialize rimsd Devices

```bash
# Run the block storage demo CLI
cargo run --example block_storage_demo

# Commands:
#   init          — initialize connected SD cards as rimsd devices
#   w <text>      — dissolve text across devices
#   r <id>        — crystallize a block back from shards
#   t <id>        — test fault tolerance (simulate lost devices)
#   s             — storage stats across all devices
```

## Monorepo Layout

```
packages/
  soradyne_core/       Rust: protocol, crypto, storage, BLE, CRDT
  soradyne_flutter/    Flutter plugin (FFI bridge to soradyne_core)
  giantt_core/         Dart: task dependency graph engine
  ai_chat_flutter/     Flutter: AI chat with action calling
apps/
  soradyne_demo/       Flutter demo (pairing, flow sync, albums)
  giantt/              Flutter task management app
  inventory/           Flutter personal inventory app
```

## Key Concepts

- **Capsule** — a trust group of devices. Established via BLE pairing with cryptographic verification. Holds shared encryption keys.
- **Piece** — one device in a capsule, with a role (Full or Accessory) and unique identity.
- **Self-Data Flow** — a fluid user-owned data item (imagine a photo album or a live heartbeat from a smart ring) that syncs peer-to-peer across capsule members.
- **Dissolution** — splitting encrypted data across physical devices using erasure coding and secret sharing. No single device holds enough to reconstruct.
- **Crystallization** — recombining shards from a threshold of devices to recover the original data.
- **.rimsd** — a directory on an SD card or flash storage device initialized for use with soradyne's dissolution protocol.

## License

This project is licensed under the MIT License — see the LICENSE file for details.

---

<div align="center">
  <sub>a <b>rim</b> project — <a href="https://instagram.com/reclaim.intimate.mutuality">@reclaim.intimate.mutuality</a></sub>
</div>
