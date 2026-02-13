# Capsule & Ensemble Implementation Plan

**Goal**: Build and persist a 3-piece capsule (2020 MacBook, simulated BLE accessory, Android phone over real BLE), discover subsets as ensembles, synchronize BLE connection info between pieces, and implement a flow type with drip-host-assignment policy. The concrete target is syncing Giantt task state and Inventory state between devices as the first demo, followed by photo flows and photo album flows.

**Target topology**:
```
  [2020 MacBook]  <-- simulated BLE -->  [Simulated Accessory]
       |
       +--------- real BLE ---------->  [Android Phone]
```

All three are pieces in one capsule. When any 2+ come online, they form an ensemble and share connection info so the third can be reached. (An ESP32-S3 running Rust is to replace the simulated BLE accessory next, making code organization to ensure at least some functional subset of the codebase compilable for and compatible with microcontrollers a priority for the current implementation. This hardware development path could eventually lead to parures — matched sets of custom hardware that form pre-solved cliques in any ensemble they join. iPad is bookmarked as a next device with a UI, making iOS support a future priority.)

**Concrete demo targets** (in priority order):
1. **Giantt sync**: Giantt task state synced between Mac and Android via DripHostedFlow — already uses `ConvergentDocument<GianttSchema>` with full CRDT ops, nearly flow-ready
2. **Inventory sync**: Inventory state synced between devices — also uses `ConvergentDocument<InventorySchema>`, same readiness level
3. **Image + album flows**: Three-tier architecture — image flows (query-response, neither jet nor drip), image composite flows (drip for composed image edits, stub for now), and album flows (drip for convergent shared photo collection). The album module has its own legacy CRDT (`LogCrdt` + `EditOp` + `MediaAlbum`) that will be ported to `ConvergentDocument<AlbumSchema>` with image/composite flows replacing direct media storage

---

## Table of Contents

1. [Phase 0: Cryptographic Identity Foundation](#phase-0-cryptographic-identity-foundation)
2. [Phase 1: BLE Transport Layer](#phase-1-ble-transport-layer)
3. [Phase 2: Device Topology — Capsules & Pieces](#phase-2-device-topology--capsules--pieces)
4. [Phase 3: Ensemble Discovery & Topology Sync](#phase-3-ensemble-discovery--topology-sync)
5. [Phase 4: Pairing UX in Soradyne Demo App](#phase-4-pairing-ux-in-soradyne-demo-app)
6. [Phase 5: Drip-Host-Assignment Flow Type](#phase-5-drip-host-assignment-flow-type)
7. [Phase 5.5: Giantt & Inventory Sync Demo](#phase-55-giantt--inventory-sync-demo)
8. [Phase 6: Integration Testing & End-to-End Demo](#phase-6-integration-testing--end-to-end-demo)
9. [Phase 7: Photo Flows & Album Port](#phase-7-photo-flows--album-port)
10. [Appendix A: Crate/Module Layout Decisions](#appendix-a-cratemodule-layout-decisions)
11. [Appendix B: Dependency Inventory](#appendix-b-dependency-inventory)
12. [Appendix C: Open Questions & Future Context](#appendix-c-open-questions--future-context)

---

## Phase 0: Cryptographic Identity Foundation

**Why first**: Everything — capsule membership, encrypted advertisements, pairing verification, flow authentication — depends on device identity and shared key material. The existing `identity/` module is empty stubs.

### 0.1 Device Identity Keypair

**Files**: `packages/soradyne_core/src/identity/keys.rs`, `identity/mod.rs`

Generate and persist a per-device Ed25519 signing keypair + X25519 key-agreement keypair (derived or separate). This is the cryptographic root of a "piece."

```rust
// identity/keys.rs

pub struct DeviceIdentity {
    /// Stable UUID for this device (persisted)
    pub device_id: Uuid,
    /// Ed25519 signing keypair
    signing_key: ed25519_dalek::SigningKey,
    /// X25519 static key for ECDH (derived from signing key, or independent)
    dh_key: x25519_dalek::StaticSecret,
}

impl DeviceIdentity {
    /// Generate a new identity (first boot)
    pub fn generate() -> Self;

    /// Load from persisted keystore file
    pub fn load(path: &Path) -> Result<Self, IdentityError>;

    /// Persist to keystore file (encrypted with platform keychain if available)
    pub fn save(&self, path: &Path) -> Result<(), IdentityError>;

    /// Public signing key (shared during pairing)
    pub fn verifying_key(&self) -> ed25519_dalek::VerifyingKey;

    /// Public DH key (shared during pairing for ECDH)
    pub fn dh_public(&self) -> x25519_dalek::PublicKey;

    /// Perform ECDH with a peer's public key
    pub fn dh_agree(&self, peer_public: &x25519_dalek::PublicKey) -> SharedSecret;

    /// Sign arbitrary data
    pub fn sign(&self, data: &[u8]) -> ed25519_dalek::Signature;
}
```

**Crate dependencies to add**: `ed25519-dalek`, `x25519-dalek`, `zeroize`

**Persistence**: JSON or CBOR keystore at `~/.soradyne/device_identity.json` (or platform-appropriate path via `dirs` crate). Keys stored as base64-encoded bytes. Future: wrap with platform keychain (macOS Keychain, Android Keystore).

**Relation to existing code**: The current `storage/device_identity.rs` does Bayesian fingerprinting of _storage devices_ (SD cards). That's orthogonal — this is _cryptographic_ device identity. They should coexist; the `DeviceId` type in `convergent/horizon.rs` (currently a `String`) will eventually use `DeviceIdentity.device_id.to_string()`.

### 0.2 Capsule Key Material

**Files**: `identity/capsule_keys.rs` (new)

When a capsule is created or a piece joins, shared symmetric key material is established. This is used for:
- Encrypting BLE advertisements (so only capsule members can decode them)
- Deriving per-session encryption keys for BLE connections

```rust
// identity/capsule_keys.rs

/// Shared key material for a capsule
pub struct CapsuleKeyBundle {
    /// Capsule UUID
    pub capsule_id: Uuid,
    /// Symmetric key for advertisement encryption (AES-256)
    /// Derived during capsule creation, distributed during pairing
    pub advertisement_key: [u8; 32],
    /// Identity Resolving Key (IRK) for BLE address rotation
    pub irk: [u8; 16],
    /// Epoch counter for key rotation (future: proactive refresh)
    pub epoch: u64,
}

impl CapsuleKeyBundle {
    /// Generate fresh keys for a new capsule
    pub fn generate(capsule_id: Uuid) -> Self;

    /// Derive advertisement encryption key for a specific epoch
    pub fn adv_key_for_epoch(&self, epoch: u64) -> [u8; 32];

    /// Serialize for transfer during pairing (encrypted with pairwise key)
    pub fn serialize_for_transfer(&self) -> Vec<u8>;

    /// Deserialize from pairing transfer
    pub fn deserialize_from_transfer(data: &[u8]) -> Result<Self, IdentityError>;
}
```

**Key distribution**: During pairing (Phase 4), the inviting piece sends `CapsuleKeyBundle` to the joining piece, encrypted with the pairwise ECDH-derived key. This is the "capsule secret" that enables a piece to understand advertisements and participate.

### 0.3 Auth Module

**Files**: `identity/auth.rs`

Implement `FlowAuthenticator` trait (already defined in `flow/traits.rs`) backed by `DeviceIdentity`:

```rust
pub struct DeviceAuthenticator {
    identity: Arc<DeviceIdentity>,
}

impl<T: Serialize> FlowAuthenticator<T> for DeviceAuthenticator {
    fn sign(&self, data: &T) -> Result<Vec<u8>, FlowError>;
    fn verify(&self, data: &T, signature: &[u8]) -> bool;
}
```

### 0.4 Deliverables

- [ ] `DeviceIdentity` struct with generate/load/save/sign/dh_agree
- [ ] `CapsuleKeyBundle` struct with generate/serialize/deserialize
- [ ] `DeviceAuthenticator` implementing existing `FlowAuthenticator` trait
- [ ] Unit tests for keygen, ECDH, signing/verification
- [ ] Integration with existing `DeviceId` type in convergent module

---

## Phase 1: BLE Transport Layer

**Why before topology**: Capsule-building and ensemble discovery both use BLE. We need a transport abstraction that supports both real BLE (for the phone) and simulated BLE (for the local accessory) behind a unified interface.

### 1.1 Architecture Decision: Where Does BLE Code Live?

**Decision**: Create a new module `packages/soradyne_core/src/ble/` within soradyne_core rather than a separate crate. Rationale:
- Tight coupling with identity (encrypted advertisements need capsule keys)
- Tight coupling with flow system (BLE is a stream transport)
- Avoids cross-crate dependency complexity at this stage
- Can be extracted to a separate crate later if needed

For the _real_ BLE stack on macOS/iOS, we'll use **btleplug** (cross-platform BLE crate in Rust) for central/peripheral operations. For simulated BLE, we build an in-process transport that mimics BLE semantics.

### 1.2 BLE Abstraction Layer

**Files**: `ble/mod.rs`, `ble/transport.rs`

```rust
// ble/transport.rs

/// A BLE advertisement packet (after encryption/decryption by the protocol layer)
#[derive(Clone, Debug)]
pub struct BleAdvertisement {
    /// Raw advertisement data (encrypted at the protocol layer)
    pub data: Vec<u8>,
    /// RSSI if available (real BLE only)
    pub rssi: Option<i16>,
    /// Source address (may be randomized)
    pub source_address: BleAddress,
}

/// Address type for BLE devices
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum BleAddress {
    /// Real BLE MAC or random address
    Real([u8; 6]),
    /// Simulated address (UUID-based)
    Simulated(Uuid),
}

/// A BLE connection handle for bidirectional data transfer
#[async_trait]
pub trait BleConnection: Send + Sync {
    /// Send data to the connected peer (via GATT write or notification)
    async fn send(&self, data: &[u8]) -> Result<(), BleError>;

    /// Receive data from the connected peer
    async fn recv(&self) -> Result<Vec<u8>, BleError>;

    /// Close the connection
    async fn disconnect(&self) -> Result<(), BleError>;

    /// Connection quality metrics
    fn rssi(&self) -> Option<i16>;

    /// The peer's BLE address
    fn peer_address(&self) -> &BleAddress;

    /// Is the connection still alive?
    fn is_connected(&self) -> bool;
}

/// Central role: scan for and connect to peripherals
#[async_trait]
pub trait BleCentral: Send + Sync {
    /// Start scanning for advertisements
    async fn start_scan(&self) -> Result<(), BleError>;

    /// Stop scanning
    async fn stop_scan(&self) -> Result<(), BleError>;

    /// Get stream of discovered advertisements
    fn advertisements(&self) -> broadcast::Receiver<BleAdvertisement>;

    /// Connect to a specific peripheral
    async fn connect(&self, address: &BleAddress) -> Result<Box<dyn BleConnection>, BleError>;
}

/// Peripheral role: advertise and accept connections
#[async_trait]
pub trait BlePeripheral: Send + Sync {
    /// Start advertising with the given data
    async fn start_advertising(&self, data: &[u8]) -> Result<(), BleError>;

    /// Stop advertising
    async fn stop_advertising(&self) -> Result<(), BleError>;

    /// Update advertisement data (e.g., new encrypted payload)
    async fn update_advertisement(&self, data: &[u8]) -> Result<(), BleError>;

    /// Accept incoming connections
    fn incoming_connections(&self) -> broadcast::Receiver<Box<dyn BleConnection>>;
}
```

### 1.3 Simulated BLE Transport

**Files**: `ble/simulated.rs`

An in-process BLE simulator that mimics BLE semantics: advertisements are broadcast to all registered listeners, connections are established via channels, and the API matches real BLE characteristics (MTU limits, connection intervals, etc.).

```rust
// ble/simulated.rs

/// A simulated BLE environment. Multiple SimulatedBle instances
/// connected to the same SimBleNetwork can discover each other.
pub struct SimBleNetwork {
    /// Shared bus for advertisements
    adv_tx: broadcast::Sender<BleAdvertisement>,
    /// Registry of "peripherals" accepting connections
    peripherals: Arc<Mutex<HashMap<BleAddress, mpsc::Sender<SimConnection>>>>,
}

impl SimBleNetwork {
    pub fn new() -> Self;
    /// Create a new simulated device on this network
    pub fn create_device(&self) -> SimBleDevice;
}

pub struct SimBleDevice {
    address: BleAddress,
    network: Arc<SimBleNetwork>,
}

impl BleCentral for SimBleDevice { /* ... */ }
impl BlePeripheral for SimBleDevice { /* ... */ }
```

The simulated accessory will be a `SimBleDevice` that runs in the same process as the Mac's soradyne_core instance. Data transfer uses tokio channels internally but respects BLE MTU chunking (to exercise the same code paths as real BLE).

### 1.4 Real BLE Transport (btleplug — All Hosted Platforms)

**Files**: `ble/real_ble.rs`

**Architecture decision**: BLE is owned entirely by Rust via **btleplug**, not by any frontend framework. The frontend (Flutter, Tauri, Unity, etc.) knows nothing about BLE — it interacts with Soradyne's Rust core through FFI for flow data, capsule management, and pairing, but never touches the radio. This means:

- Sync works without any UI running (headless Rust process on Mac/Linux)
- Adding a new frontend (Tauri desktop app, Unity VR app) requires zero BLE work
- The BLE stack is testable in isolation from any UI framework
- On Android, btleplug uses JNI to access the platform BLE stack directly from Rust — no Dart BLE plugin needed

btleplug supports: macOS (CoreBluetooth), iOS (CoreBluetooth), Linux (BlueZ), Windows (WinRT), and Android (JNI). For ESP32-S3, the BLE backend is `esp-idf-hal` (same `BleCentral`/`BlePeripheral` traits, different implementation).

```rust
// ble/real_ble.rs

pub struct BtleplugCentral {
    manager: btleplug::api::Manager,
    adapter: btleplug::api::Adapter,
}

impl BleCentral for BtleplugCentral { /* ... */ }

pub struct BtleplugPeripheral {
    // Peripheral role for advertising and accepting connections
    // On macOS: uses CoreBluetooth CBPeripheralManager (may need core-bluetooth crate)
    // On Android: uses BluetoothLeAdvertiser via JNI
    // On Linux: uses BlueZ D-Bus API
}

impl BlePeripheral for BtleplugPeripheral { /* ... */ }
```

**Note on peripheral role**: btleplug primarily supports the BLE central role. The peripheral role (advertising, accepting connections) may require platform-specific handling — see Appendix C.1. The recommendation is to start with Mac as central / Phone as peripheral, since this is the common BLE pattern.

**Android specifics**: btleplug's Android support uses JNI to call the Android BLE APIs directly from Rust. The Flutter app loads `libsoradyne.so` (already set up — see CLAUDE.md for `cargo ndk` cross-compilation), and the Rust runtime within that `.so` owns BLE scanning, advertising, and connections. Flutter only calls FFI functions for UI-level actions (create capsule, start pairing, get flow state). No Dart BLE plugin is involved.

### 1.5 Encrypted Advertisements

**Files**: `ble/encrypted_adv.rs`

BLE advertisements must be encrypted so only capsule members can interpret them. Adversaries see random-looking bytes.

```rust
// ble/encrypted_adv.rs

/// Advertisement payload structure (before encryption)
#[derive(Serialize, Deserialize)]
pub struct AdvertisementPayload {
    /// Capsule ID (so receiver knows which key to try)
    /// Encoded as a 4-byte truncated hash (saves space, receiver tries all known capsules)
    pub capsule_hint: [u8; 4],
    /// This piece's current device_id (truncated for space)
    pub piece_hint: [u8; 4],
    /// Nonce / sequence number (for replay protection)
    pub seq: u32,
    /// Ensemble state summary (compact)
    pub topology_hash: u32,
    /// Optional: connectivity info to share (which other pieces we can see)
    pub known_pieces: Vec<u8>,  // bitfield of known-online piece indices
}

/// Encrypt an advertisement payload using the capsule's advertisement key
pub fn encrypt_advertisement(
    payload: &AdvertisementPayload,
    capsule_keys: &CapsuleKeyBundle,
) -> Vec<u8>;

/// Try to decrypt an advertisement against all known capsule keys
/// Returns None if the advertisement doesn't match any known capsule
pub fn try_decrypt_advertisement(
    raw: &[u8],
    known_capsules: &[CapsuleKeyBundle],
) -> Option<(Uuid, AdvertisementPayload)>;
```

**Encryption scheme**: AES-128-CCM (standard BLE encryption primitive, compact, AEAD). The capsule's `advertisement_key` is the key; a 13-byte nonce includes the `seq` counter + a device-specific salt. The 4-byte `capsule_hint` is transmitted in the clear as part of the BLE advertisement's manufacturer-specific data to help the receiver quickly select which key to try without exhaustive search.

**Target: BLE 5.0** (all target devices support it: 2020 MacBook, modern Android phones, ESP32-S3). BLE 4.x is not supported.

**Advertisement vs. connection**: Advertisements are only for discovery — "I exist, I'm in your capsule." All real data transfer (topology sync, capsule gossip, flow data, pairing) happens over GATT connections after discovery. Even BLE 5.0 extended advertisements max out at ~255 bytes per PDU (~1650 with chaining), so the architecture is connection-based regardless.

**BLE 5.0 extended advertisement space budget** (~255 bytes available):
- 2 bytes: AD type header (manufacturer specific)
- 2 bytes: company ID (can use 0xFFFF for development)
- 4 bytes: capsule_hint (cleartext)
- 4 bytes: nonce/seq
- Encrypted payload (remaining ~240 bytes available):
  - 4 bytes: piece_hint
  - 4 bytes: topology_hash
  - Variable: known_pieces bitfield
  - 4 bytes: MIC (authentication tag)
  - Ample room for future fields (battery level, capabilities summary, etc.)

The lean beacon design means advertisements are fast to encrypt/decrypt and cheap to broadcast. Heavy data moves over GATT connections (MTU negotiable up to 512 bytes, unlimited writes).

### 1.6 GATT Service Definition

**Files**: `ble/gatt.rs`

Define a custom GATT service for Soradyne data transfer. **All non-pairing data flows through a unified routed message envelope** — the GATT layer doesn't distinguish topology sync from flow data. This is critical for multi-hop routing: an intermediate piece forwarding a message doesn't need to know whether it's a topology update or a flow operation.

```
Service: Soradyne Rim Protocol
UUID: [custom 128-bit UUID based on soradyne namespace]

Characteristics:
  - Routed Message (read/write/notify)
    UUID: [custom]
    Used for: ALL data transfer (topology sync, flow data, ensemble messages).
    Payload format: RoutedEnvelope (see below).

  - Pairing Exchange (write/indicate)
    UUID: [custom]
    Used for: pairing protocol messages (ECDH, PIN verification).
    Not routed — pairing is always point-to-point over a direct BLE connection.
```

```rust
// ble/gatt.rs

/// Every non-pairing message sent over BLE uses this envelope.
/// This is the unit of forwarding: an intermediate piece routes
/// the entire envelope without inspecting the payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutedEnvelope {
    /// Who originally sent this message
    pub source: Uuid,
    /// Who should ultimately receive it (Uuid::nil = broadcast to all)
    pub destination: Uuid,
    /// Decremented at each hop; drop if 0 (prevents routing loops)
    pub ttl: u8,
    /// What kind of message this is (so the destination can dispatch)
    pub message_type: MessageType,
    /// The actual payload (opaque to intermediate pieces)
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MessageType {
    /// Topology sync (ensemble topology updates, peer introductions)
    TopologySync,
    /// Flow data (operations, horizon exchange, state sync)
    FlowSync,
    /// Capsule gossip (capsule set union during sync)
    CapsuleGossip,
}

impl RoutedEnvelope {
    pub fn new_unicast(source: Uuid, destination: Uuid, message_type: MessageType, payload: Vec<u8>) -> Self {
        Self { source, destination, ttl: 8, message_type, payload }
    }

    pub fn new_broadcast(source: Uuid, message_type: MessageType, payload: Vec<u8>) -> Self {
        Self { source, destination: Uuid::nil(), ttl: 8, message_type, payload }
    }

    /// Should this message be forwarded to a given peer?
    pub fn should_forward_to(&self, peer_id: &Uuid) -> bool {
        self.ttl > 0
            && *peer_id != self.source  // don't send back to originator
            && (self.destination == Uuid::nil() || self.destination == *peer_id)
    }

    /// Create a forwarded copy with decremented TTL
    pub fn forwarded(&self) -> Option<Self> {
        if self.ttl == 0 { return None; }
        let mut copy = self.clone();
        copy.ttl -= 1;
        Some(copy)
    }
}
```

**Design note**: The `RoutedEnvelope` is the atom of the transport layer. A piece receiving an envelope either processes it (if `destination` matches its own UUID or is broadcast) or forwards it (if it has a route to the destination). This forwarding is entirely mechanical — no flow-level understanding needed. See Phase 3.4 for the `EnsembleMessenger` that builds on this.

### 1.7 Deliverables

- [ ] `BleCentral`, `BlePeripheral`, `BleConnection` trait definitions
- [ ] `SimBleNetwork` + `SimBleDevice` simulated transport
- [ ] `BtleplugCentral` + `BtleplugPeripheral` real BLE transport (macOS + Android via btleplug)
- [ ] `encrypt_advertisement` / `try_decrypt_advertisement`
- [ ] GATT service UUID definitions + `RoutedEnvelope` message format
- [ ] Unit tests for simulated BLE (advertisement round-trip, connection, data transfer)
- [ ] Unit test for advertisement encryption/decryption
- [ ] Unit test for `RoutedEnvelope` forwarding logic (TTL decrement, source filtering, broadcast)

---

## Phase 2: Device Topology — Capsules & Pieces

**Why before ensemble**: Capsules are the persistent structure; ensembles are the dynamic runtime. We need capsule persistence and piece authorization before we can track who's online.

### 2.1 Capsule Data Model

**Files**: `packages/soradyne_core/src/topology/mod.rs`, `topology/capsule.rs`

```rust
// topology/capsule.rs

/// A persistent capsule — a curated set of authorized pieces
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capsule {
    /// Unique capsule identifier
    pub id: Uuid,
    /// Human-friendly name (e.g., "Dana's personal devices")
    pub name: String,
    /// When this capsule was created
    pub created_at: DateTime<Utc>,
    /// Authorized pieces (the "capsule wardrobe")
    pub pieces: Vec<PieceRecord>,
    /// Key material for this capsule
    pub keys: CapsuleKeyBundle,
    /// Capsule status
    pub status: CapsuleStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapsuleStatus {
    Active,
    /// Retired capsules are kept for historical reference but not used
    Retired { retired_at: DateTime<Utc> },
}

/// Record of an authorized piece within a capsule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PieceRecord {
    /// The piece's device UUID
    pub device_id: Uuid,
    /// Human-friendly name (e.g., "Dana's MacBook", "Ring accessory")
    pub name: String,
    /// The piece's Ed25519 public verifying key
    pub verifying_key: Vec<u8>,
    /// The piece's X25519 public DH key
    pub dh_public_key: Vec<u8>,
    /// When this piece was added to the capsule
    pub added_at: DateTime<Utc>,
    /// Piece capabilities
    pub capabilities: PieceCapabilities,
    /// Piece role category
    pub role: PieceRole,
}

/// What a piece can do in the capsule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PieceCapabilities {
    /// Can this piece host drip streams? (i.e., run CRDT merge + serve state)
    pub can_host_drip: bool,
    /// Can this piece memorize (store) flow data?
    pub can_memorize: bool,
    /// Can this piece route traffic between other pieces? (transport-layer,
    /// not a flow role — all pieces route by default, this flag is for
    /// constrained devices that genuinely cannot)
    pub can_route: bool,
    /// Does this piece have a user interface?
    pub has_ui: bool,
    /// Approximate storage capacity (bytes, 0 = unknown)
    pub storage_bytes: u64,
    /// Approximate battery level tracking?
    pub battery_aware: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PieceRole {
    /// Full piece: runs Soradyne core, participates in topology
    Full,
    /// Accessory: minimal interface, singular role
    Accessory,
}
```

**For the 3-piece capsule**:
| Piece | can_host_drip | can_memorize | can_route | role |
|-------|--------------|-------------|-----------|------|
| Mac | true | true | true | Full |
| Android phone | true | true | true | Full |
| Simulated Accessory | false | true | true | Accessory |

### 2.2 Capsule Storage (Add-Only Set, Not a CRDT)

**Design decision**: Capsules are **not** implemented as `ConvergentDocument` or as flows. They are pre-flow infrastructure — simpler, more fundamental, and with no dependency on the flow system.

A capsule's piece set is **add-only**: pieces are added but never individually removed (per the rim-protocol spec, capsules are "retired whole"). This means the merge function is just **set union** — the simplest possible convergent data structure. There's no need for:
- Operation logs or sequence numbers
- Horizons or causal tracking
- Informed-remove semantics
- Latest-wins field resolution
- Materialization from operations

Two devices connect, they each share their piece set, they take the union, done. If two pieces independently add a third, both end up with all three — no conflict to resolve.

**Why not ConvergentDocument**: The CRDT engine is designed for frequently-changing data with concurrent conflicting edits (Giantt tasks, inventory items). A capsule changes a handful of times in its lifetime and then is static. Using ConvergentDocument would create a dependency inversion (flows depend on capsules, so capsules shouldn't be built on flows) and force accessories like ESP32s to run the full CRDT engine just to know who their peers are.

**Why not a flow**: Capsules are what flows _need to exist_ before they can work — they tell the flow system who the participants are and what keys to use. Making capsules a flow creates a bootstrapping dependency: you'd need a working flow evaluation to load the capsule that tells you who to sync flows with. Local-first evaluation technically solves this, but it's unnecessary fragility for something this simple.

**Files**: `topology/capsule_store.rs`

```rust
// topology/capsule_store.rs

/// A capsule is an add-only set of piece records plus key material.
/// Sync is trivial: gossip the full set, merge by union.
/// Stored as a single serialized file per capsule.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capsule {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub pieces: Vec<PieceRecord>,      // Add-only; merge = union by device_id
    pub flows: Vec<FlowConfig>,        // Add-only; flows registered in this capsule
    pub keys: CapsuleKeyBundle,
    pub status: CapsuleStatus,
}

impl Capsule {
    /// Merge another copy of this capsule (from a peer).
    /// Pure set union on pieces and flows — any entry either of us knows about,
    /// we both know.
    pub fn merge(&mut self, other: &Capsule) {
        for piece in &other.pieces {
            if !self.pieces.iter().any(|p| p.device_id == piece.device_id) {
                self.pieces.push(piece.clone());
            }
        }
        for flow in &other.flows {
            if !self.flows.iter().any(|f| f.id == flow.id) {
                self.flows.push(flow.clone());
            }
        }
    }

    /// Serialize for gossip (CBOR for compactness, works on ESP32 too)
    pub fn to_bytes(&self) -> Vec<u8>;

    /// Deserialize from gossip
    pub fn from_bytes(data: &[u8]) -> Result<Self, TopologyError>;
}

/// On-device storage for capsules this device belongs to.
/// Each capsule is a single file: {capsule_id}.capsule (CBOR or JSON)
pub struct CapsuleStore {
    data_dir: PathBuf,
    capsules: HashMap<Uuid, Capsule>,
}

impl CapsuleStore {
    pub fn load(data_dir: &Path) -> Result<Self, TopologyError>;
    pub fn save(&self) -> Result<(), TopologyError>;
    pub fn create_capsule(&mut self, name: &str, keys: CapsuleKeyBundle) -> Result<Uuid, TopologyError>;
    pub fn add_piece(&mut self, capsule_id: Uuid, piece: PieceRecord) -> Result<(), TopologyError>;
    pub fn get_capsule(&self, capsule_id: &Uuid) -> Option<&Capsule>;
    pub fn list_capsules(&self) -> Vec<&Capsule>;
    pub fn retire_capsule(&mut self, capsule_id: &Uuid) -> Result<(), TopologyError>;
    /// Merge a peer's copy of a capsule (set union)
    pub fn merge_from_peer(&mut self, peer_capsule: &Capsule) -> Result<(), TopologyError>;
}
```

**Sync protocol**: When two pieces connect via BLE (Phase 3), one of the first messages exchanged is their capsule state. Each side calls `capsule.merge(&peer_capsule)` — a trivially cheap operation. This happens once at connection time and again if either side adds a new piece (rare).

**Accessory compatibility**: Since the capsule is a flat serialized struct (no CRDT engine, no operation log), an ESP32 accessory can deserialize and use it directly. The format used for gossip between full pieces is the _same_ format stored on the accessory. No "snapshot" conversion needed.

**Persistence**: One file per capsule. JSON for development, CBOR (via `ciborium`) for production/embedded. Both are trivially readable on any platform.

### 2.3 Deliverables

- [ ] `Capsule` struct with `merge()` (set union by device_id)
- [ ] `PieceRecord`, `PieceCapabilities`, `PieceRole` data structures
- [ ] `CapsuleStore` with create/add/list/retire/merge_from_peer
- [ ] CBOR serialization for compact gossip (compatible with `no_std` deserialization on ESP32)
- [ ] Persistence to disk (one file per capsule)
- [ ] Unit tests: two devices independently add a third → merge → both have all three
- [ ] Unit test: merge is idempotent (merging the same capsule twice is a no-op)

---

## Phase 3: Ensemble Discovery & Topology Sync

This is where pieces find each other at runtime and share connectivity information.

### 3.1 Ensemble Manager

**Files**: `topology/ensemble.rs`

```rust
// topology/ensemble.rs

/// The live ensemble — runtime tracking of which pieces are online
pub struct EnsembleManager {
    /// Which capsule this ensemble is scoped to
    capsule_id: Uuid,
    /// Our device identity
    device_identity: Arc<DeviceIdentity>,
    /// Capsule key material (for encrypted advertisements)
    capsule_keys: CapsuleKeyBundle,
    /// Currently known online pieces and their connectivity
    topology: Arc<RwLock<EnsembleTopology>>,
    /// BLE transport handles
    ble_central: Box<dyn BleCentral>,
    ble_peripheral: Box<dyn BlePeripheral>,
}

/// The ensemble's connectivity graph — a **directed multigraph**.
///
/// Directed: edge A→B doesn't imply B→A (asymmetric connectivity is real:
///   a BLE peripheral can be discovered by a central but not vice versa;
///   bandwidth may differ by direction).
/// Multi: multiple edges can exist between the same pair of pieces
///   (e.g., A→B over BLE direct AND A→B relayed through C), representing
///   different transport types with different quality characteristics.
#[derive(Clone, Debug)]
pub struct EnsembleTopology {
    /// Known online pieces (device_id -> last seen, connection quality)
    pub online_pieces: HashMap<Uuid, PiecePresence>,
    /// Directed edges: who can reach whom, via what transport.
    /// Multiple edges between the same (from, to) pair are allowed
    /// (different transport types).
    pub edges: Vec<TopologyEdge>,
    /// Our local view timestamp
    pub last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct PiecePresence {
    pub device_id: Uuid,
    pub last_advertisement: DateTime<Utc>,
    pub last_data_exchange: Option<DateTime<Utc>>,
    pub rssi: Option<i16>,
    /// How we can reach this piece. None = seen in advertisement only,
    /// not yet connected. Direct = we have a BLE connection to them.
    /// Indirect = we can reach them through one or more intermediaries.
    /// Presence is determined by reachability in the topology multigraph,
    /// NOT by having a direct BLE connection.
    pub reachability: PieceReachability,
}

#[derive(Clone, Debug)]
pub enum PieceReachability {
    /// Seen in advertisement but no data path established yet
    AdvertisementOnly,
    /// We have a direct BLE connection to this piece
    Direct,
    /// Reachable through one or more intermediary pieces
    Indirect {
        /// Next hop toward this piece (the piece we'd hand a message to)
        next_hop: Uuid,
        /// Estimated hop count
        hop_count: u8,
    },
}

/// A directed edge in the topology multigraph.
/// from→to means "from can send data to to via this transport."
/// The reverse direction (to→from) is a separate edge with potentially
/// different quality characteristics (asymmetric bandwidth is common in BLE).
#[derive(Clone, Debug)]
pub struct TopologyEdge {
    pub from: Uuid,
    pub to: Uuid,
    pub transport: TransportType,
    pub quality: ConnectionQuality,
}

#[derive(Clone, Debug)]
pub enum TransportType {
    BleDirect,
    BleRelayed { via: Uuid },
    SimulatedBle,
}

#[derive(Clone, Debug)]
pub struct ConnectionQuality {
    pub rssi: Option<i16>,
    pub latency_ms: Option<u32>,
    pub bandwidth_estimate: Option<u32>,  // bytes/sec
}
```

### 3.2 Discovery Loop

The ensemble manager runs a continuous discovery loop:

```
loop {
    1. Broadcast encrypted advertisement (our piece_hint, topology_hash, known_pieces)
    2. Scan for advertisements from other pieces
    3. For each decrypted advertisement from a capsule peer:
       a. Update their presence in our topology
       b. If we're not connected and should be: initiate BLE connection
       c. If their topology_hash differs from ours: schedule topology sync
    4. For connected pieces: exchange topology updates
    5. Propagate learned connectivity info:
       - "I can see piece C" → tell piece B (so B knows C is reachable through us)
    6. Sleep for scan_interval (e.g., 1–5 seconds)
}
```

### 3.3 Topology Synchronization Protocol

When two pieces connect, they sync their view of who's online:

```rust
/// Messages exchanged during topology sync
#[derive(Serialize, Deserialize)]
pub enum TopologySyncMessage {
    /// "Here's my current topology view"
    TopologyUpdate {
        /// Pieces I can see directly
        direct_peers: Vec<PeerInfo>,
        /// Pieces I've heard about from others
        indirect_peers: Vec<PeerInfo>,
        /// My topology hash (for quick comparison)
        topology_hash: u32,
    },
    /// "I have connection info for piece X that you might need"
    PeerIntroduction {
        /// The piece being introduced
        piece_id: Uuid,
        /// BLE address info for reaching them
        ble_address: Option<BleAddress>,
        /// Their last known advertisement data
        last_advertisement: Option<Vec<u8>>,
        /// Connectivity quality from the introducer
        quality: ConnectionQuality,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub device_id: Uuid,
    pub ble_address: Option<BleAddress>,
    pub last_seen: DateTime<Utc>,
    pub capabilities: PieceCapabilities,
}
```

**The key scenario**: Mac and Android phone are connected. Mac also has a simulated BLE connection to the Accessory. Mac sends a `PeerIntroduction` to Android phone about the Accessory (and vice versa). Now Android phone knows the Accessory exists and its BLE address. If the Accessory comes into real BLE range of the Android phone, the Android phone can recognize it. If not, the Android phone can route through the Mac.

### 3.4 EnsembleMessenger — Logical Routing Layer

**Files**: `topology/messenger.rs`

The `EnsembleMessenger` is the layer between raw BLE connections and everything above (topology sync, flow sync, capsule gossip). It provides **logical addressing**: callers send to a destination UUID, and the messenger figures out the route — direct BLE if available, multi-hop through intermediaries if not. This is the key abstraction that makes multi-hop routing invisible to flows.

```rust
// topology/messenger.rs

use super::ensemble::{EnsembleTopology, PieceReachability};
use super::super::ble::gatt::RoutedEnvelope;

/// Logical messaging layer — hides multi-hop routing from flows.
///
/// The EnsembleMessenger is to the flow layer what IP is to TCP:
/// callers address messages to a destination, and the messenger
/// handles routing, forwarding, and TTL management. Flows never
/// see BLE connections, hop counts, or routing tables.
#[async_trait]
pub trait EnsembleMessenger: Send + Sync {
    /// Send data to a specific piece, routing through intermediaries if needed.
    /// Returns Ok(()) when the message has been handed to the next hop
    /// (NOT when the destination has received it — this is best-effort,
    /// like UDP. Reliable delivery is the flow layer's concern).
    async fn send_to(
        &self,
        destination: Uuid,
        message_type: MessageType,
        payload: &[u8],
    ) -> Result<(), RoutingError>;

    /// Broadcast data to all pieces in the ensemble.
    /// Each piece that receives the broadcast forwards it to its neighbors
    /// (with decremented TTL) so it propagates through the mesh.
    async fn broadcast(
        &self,
        message_type: MessageType,
        payload: &[u8],
    ) -> Result<(), RoutingError>;

    /// Subscribe to incoming messages addressed to us (from any source,
    /// however many hops they traversed). The source UUID in each message
    /// is the original sender, not the last hop.
    fn incoming(&self) -> broadcast::Receiver<RoutedEnvelope>;

    /// Check if a piece is currently reachable (directly or indirectly).
    fn is_reachable(&self, destination: &Uuid) -> bool;

    /// Get the reachability info for a piece.
    fn reachability(&self, destination: &Uuid) -> Option<PieceReachability>;
}

#[derive(Debug)]
pub enum RoutingError {
    /// No route to the destination (not in topology, or all paths down)
    Unreachable(Uuid),
    /// Message exceeded TTL (routing loop detected and broken)
    TtlExceeded,
    /// BLE transport error on the next hop
    TransportError(BleError),
}

/// Implementation of EnsembleMessenger backed by the ensemble topology.
pub struct TopologyMessenger {
    /// Our device ID (source for outgoing messages)
    device_id: Uuid,
    /// The live topology (consulted for routing decisions)
    topology: Arc<RwLock<EnsembleTopology>>,
    /// Direct BLE connections to neighboring pieces
    /// (the messenger manages these, not individual flows)
    connections: Arc<RwLock<HashMap<Uuid, Arc<dyn BleConnection>>>>,
    /// Channel for incoming messages (after routing/forwarding)
    incoming_tx: broadcast::Sender<RoutedEnvelope>,
}

impl TopologyMessenger {
    /// The forwarding loop: runs on each incoming message from any BLE connection.
    /// This is the "IP router" behavior — entirely mechanical, no content inspection.
    async fn handle_incoming(&self, envelope: RoutedEnvelope) {
        // 1. Is this message for us?
        let for_us = envelope.destination == self.device_id
            || envelope.destination == Uuid::nil(); // broadcast

        if for_us {
            // Deliver to local subscribers
            let _ = self.incoming_tx.send(envelope.clone());
        }

        // 2. Should we forward? (broadcast, or unicast not for us)
        if envelope.destination != self.device_id {
            if let Some(forwarded) = envelope.forwarded() {
                // Find the right next hop(s)
                let connections = self.connections.read().unwrap();
                for (peer_id, conn) in connections.iter() {
                    if forwarded.should_forward_to(peer_id) {
                        let data = serialize(&forwarded);
                        let _ = conn.send(&data).await;
                    }
                }
            }
        }
    }

    /// Compute next hop for a unicast message by consulting the topology.
    fn next_hop_for(&self, destination: &Uuid) -> Option<Uuid> {
        let topology = self.topology.read().unwrap();
        match topology.online_pieces.get(destination)?.reachability {
            PieceReachability::Direct => Some(*destination),
            PieceReachability::Indirect { next_hop, .. } => Some(next_hop),
            PieceReachability::AdvertisementOnly => None,
        }
    }
}
```

**Why this layer matters**: Without `EnsembleMessenger`, every consumer of BLE data (topology sync, capsule gossip, flow sync) would need to manage its own routing, maintain its own connection map, and implement its own forwarding. With it, flows call `messenger.send_to(peer_id, FlowSync, ops)` and the messenger does the rest — whether the peer is one hop away or three. Adding a new transport type (e.g., WiFi Direct, TCP over LAN) requires only adding a new connection type to the messenger's connection map, not changing any flow code.

### 3.5 Connection Info Propagation

This is the "if 2 connect and 1 has info about the 3rd" requirement. Implemented as topology sync messages routed through the `EnsembleMessenger`:

```rust
impl EnsembleManager {
    /// After connecting to a new piece, share what we know about other pieces.
    /// Uses EnsembleMessenger so introductions can propagate through multi-hop routes.
    async fn propagate_peer_info(&self, new_peer_id: Uuid) {
        let topology = self.topology.read().unwrap();
        for (piece_id, presence) in &topology.online_pieces {
            if *piece_id != new_peer_id {
                let intro = TopologySyncMessage::PeerIntroduction {
                    piece_id: *piece_id,
                    ble_address: presence.ble_address.clone(),
                    last_advertisement: presence.last_adv_data.clone(),
                    quality: presence.quality.clone(),
                };
                // Send via messenger — works whether the new peer is direct or routed
                self.messenger.send_to(
                    new_peer_id,
                    MessageType::TopologySync,
                    &serialize(&intro),
                ).await?;
            }
        }
    }

    /// When we learn about a new piece from a peer, update our topology.
    /// If the piece is not directly reachable, mark it as Indirect with
    /// the introducing peer as the next hop.
    fn handle_peer_introduction(&self, from_peer: Uuid, intro: PeerIntroduction) {
        let mut topology = self.topology.write().unwrap();
        if !topology.online_pieces.contains_key(&intro.piece_id) {
            topology.online_pieces.insert(intro.piece_id, PiecePresence {
                device_id: intro.piece_id,
                last_advertisement: intro.last_seen,
                last_data_exchange: None,
                rssi: None,
                reachability: PieceReachability::Indirect {
                    next_hop: from_peer,
                    hop_count: 1,  // one hop through the introducing peer
                },
            });
        }
    }
}
```

### 3.6 Deliverables

- [ ] `EnsembleManager` with discovery loop
- [ ] `EnsembleTopology` as a directed multigraph (multiple transports per pair, asymmetric edges)
- [ ] `PieceReachability` enum (AdvertisementOnly / Direct / Indirect with next_hop)
- [ ] `RoutedEnvelope` message format with source, destination, TTL, message_type
- [ ] `EnsembleMessenger` trait with `send_to()`, `broadcast()`, `incoming()`
- [ ] `TopologyMessenger` implementation with forwarding loop (the "IP router" behavior)
- [ ] `TopologySyncMessage` protocol
- [ ] Peer introduction / connection info propagation (via EnsembleMessenger)
- [ ] Integration test: 3-device simulated ensemble where A↔B and A↔C, then B learns about C through A
- [ ] Integration test: A sends flow data to C routed through B (multi-hop)
- [ ] Timeout/stale-piece detection (piece goes offline → remove from topology after configurable interval)

---

## Phase 4: Pairing UX in Soradyne Demo App

"Sort of like Signal, but using BLE idioms."

### 4.1 Pairing Protocol (Rust side)

**Files**: `topology/pairing.rs`

The pairing protocol establishes trust between two pieces and adds one to the other's capsule. Modular design: the core protocol is abstract over the out-of-band verification method.

```rust
// topology/pairing.rs

/// Pairing protocol states (state machine)
#[derive(Debug)]
pub enum PairingState {
    /// Waiting for a peer to be discovered
    Scanning,
    /// Found a peer, starting ECDH
    Discovered { peer_address: BleAddress },
    /// ECDH complete, awaiting out-of-band verification
    AwaitingVerification {
        shared_secret: [u8; 32],
        /// 6-digit PIN derived from shared secret for user comparison
        verification_pin: String,
        peer_public_key: Vec<u8>,
    },
    /// User confirmed PIN match
    Verified,
    /// Capsule key material exchanged successfully
    Complete { peer_device_id: Uuid },
    /// Pairing failed
    Failed { reason: String },
}

/// The pairing protocol engine
pub struct PairingEngine {
    device_identity: Arc<DeviceIdentity>,
    capsule_store: Arc<Mutex<CapsuleStore>>,
    state: Arc<RwLock<PairingState>>,
}

impl PairingEngine {
    /// Start pairing as the inviter (capsule already exists)
    pub async fn start_invite(
        &self,
        capsule_id: Uuid,
        ble: &dyn BleCentral,
    ) -> Result<(), PairingError>;

    /// Start pairing as the joiner (scanning for inviter)
    pub async fn start_join(
        &self,
        ble: &dyn BleCentral,
    ) -> Result<(), PairingError>;

    /// User confirms the PIN matches
    pub async fn confirm_pin(&self) -> Result<(), PairingError>;

    /// User rejects the PIN
    pub fn reject_pin(&self);

    /// Get current pairing state (for UI)
    pub fn state(&self) -> PairingState;
}
```

**Pairing flow**:
```
Inviter (Mac)                           Joiner (Android phone)
─────────────                           ────────────────
1. Enters "Add piece" mode              1. Enters "Join capsule" mode
2. Starts BLE advertising with          2. Starts BLE scanning for
   pairing service UUID                    pairing service UUID

3. ←── BLE Connection Established ───→

4. Sends DH public key ──────────────→
                                        5. Sends DH public key
←──────────────────────────────────────

6. Both compute shared secret via ECDH
   Both derive 6-digit PIN from shared secret

7. UI shows PIN: "742819"              7. UI shows PIN: "742819"
   "Does this match?"                     "Does this match?"

8. User confirms on both devices

9. Inviter sends (encrypted with
   shared secret):
   - CapsuleKeyBundle
   - Existing PieceRecords
   - Capsule metadata
   ──────────────────────────────────→  10. Joiner decrypts, stores capsule

11. Joiner sends (encrypted):
    - Own PieceRecord
    (name, public keys, capabilities)
←────────────────────────────────────

12. Both add each other's PieceRecord
    to their capsule (add to set, persist)

13. Pairing complete. Both now have:
    - Capsule key material
    - Each other's identity
    - Can decode each other's advertisements
```

### 4.2 Verification Method Abstraction

The PIN check is one verification method. The design is modular:

```rust
/// Trait for out-of-band verification methods
pub trait PairingVerifier: Send + Sync {
    /// Derive verification data from shared secret
    fn derive_challenge(&self, shared_secret: &[u8; 32]) -> VerificationChallenge;

    /// Check if verification succeeded
    fn verify(&self, challenge: &VerificationChallenge, user_input: &str) -> bool;
}

pub enum VerificationChallenge {
    /// 6-digit numeric PIN displayed on both devices
    NumericPin(String),
    /// QR code displayed on one device, scanned by other (future)
    QrCode(Vec<u8>),
    /// NFC tap (future)
    NfcExchange(Vec<u8>),
}

/// Default: 6-digit numeric comparison
pub struct NumericPinVerifier;

impl PairingVerifier for NumericPinVerifier {
    fn derive_challenge(&self, shared_secret: &[u8; 32]) -> VerificationChallenge {
        // SHA256(shared_secret || "soradyne-pin-v1") → take first 3 bytes → mod 1000000
        let pin = derive_pin(shared_secret);
        VerificationChallenge::NumericPin(format!("{:06}", pin))
    }

    fn verify(&self, challenge: &VerificationChallenge, user_input: &str) -> bool {
        matches!(challenge, VerificationChallenge::NumericPin(pin) if pin == user_input)
    }
}
```

### 4.3 Flutter UI for Pairing

**Files**: `apps/soradyne_demo/flutter_app/lib/screens/pairing/`

New screens in the demo app:

1. **CapsuleListScreen** — Shows existing capsules, button to create new one
2. **CapsuleDetailScreen** — Shows pieces in a capsule, "Add piece" button
3. **PairingInviteScreen** — "Waiting for device..." with animated BLE scan indicator
4. **PairingJoinScreen** — "Scanning for invitation..." with BLE scan indicator
5. **PinVerificationScreen** — Shows 6-digit PIN, "Does this match the other device?" with Confirm/Reject buttons
6. **PairingCompleteScreen** — Success state, shows new piece info

**UX flow** (inspired by Signal's device linking):
```
Capsule Screen → "Add piece" →
  Choice: "This device wants to invite" / "This device wants to join"

  If inviting:
    → PairingInviteScreen (scanning animation)
    → Device found → PinVerificationScreen
    → User confirms → PairingCompleteScreen

  If joining:
    → PairingJoinScreen (scanning animation)
    → Invitation found → PinVerificationScreen
    → User confirms → PairingCompleteScreen
```

### 4.4 FFI Functions for Pairing

```rust
// ffi/pairing_bridge.rs

extern "C" fn soradyne_create_capsule(name: *const c_char) -> *const c_char;
extern "C" fn soradyne_list_capsules() -> *const c_char;
extern "C" fn soradyne_start_pairing_invite(capsule_id: *const c_char) -> i32;
extern "C" fn soradyne_start_pairing_join() -> i32;
extern "C" fn soradyne_get_pairing_state() -> *const c_char;
extern "C" fn soradyne_confirm_pairing_pin() -> i32;
extern "C" fn soradyne_reject_pairing_pin() -> i32;
extern "C" fn soradyne_get_capsule_pieces(capsule_id: *const c_char) -> *const c_char;
```

### 4.5 Simulated Accessory Pairing

For the simulated accessory, pairing happens in-process without real BLE. A CLI command or demo app button triggers:

```rust
/// Create a simulated accessory and pair it into a capsule
pub async fn create_simulated_accessory(
    capsule_store: &mut CapsuleStore,
    capsule_id: Uuid,
    sim_network: &SimBleNetwork,
    name: &str,
) -> Result<(DeviceIdentity, SimBleDevice), TopologyError> {
    // 1. Generate identity for the accessory
    // 2. Create a SimBleDevice on the sim network
    // 3. Exchange keys (in-process, no real ECDH needed - trust is implicit)
    // 4. Add PieceRecord to capsule with Accessory role
    // 5. Return identity + device handle so the accessory can participate
}
```

### 4.6 Deliverables

- [ ] `PairingEngine` state machine with ECDH + PIN verification
- [ ] `PairingVerifier` trait with `NumericPinVerifier` implementation
- [ ] FFI functions for pairing lifecycle
- [ ] Flutter pairing screens (CapsuleList, CapsuleDetail, PairingInvite, PairingJoin, PinVerification, PairingComplete)
- [ ] Simulated accessory creation/pairing helper
- [ ] Integration test: full pairing flow between two simulated devices

---

## Phase 5: Drip-Host-Assignment Flow Type

This is the capstone: a flow type that uses policies (informed by ensemble topology) to decide which piece hosts the drip stream, and what happens when the host drops.

### 5.1 Flow Type Definition

**Files**: `packages/soradyne_core/src/flow/types/drip_hosted.rs` (new directory `flow/types/`)

```rust
// flow/types/drip_hosted.rs

/// A flow type where one piece hosts the authoritative drip stream.
/// The host is selected by policy based on ensemble topology.
/// Other pieces can read the drip; writes go through the host.
/// If the host drops, policy dictates failover.
///
/// Streams:
///   - "state" (drip, singleton): The authoritative convergent state
///   - "edits" (jet, per-party): Edit operations from each participant
///   - "host_assignment" (drip, singleton): Who is currently hosting
///
/// Roles:
///   - DripHost: Runs the CRDT merge, serves the authoritative state
///   - Editor: Sends edit operations, reads drip state
///   - Memorizer: Stores snapshots/operations (can be same piece as host)
///
/// Note: Relaying/routing is NOT a flow role — it's a transport-layer concern
/// handled by the EnsembleManager. All pieces route traffic transparently,
/// like IP routers, without knowledge of flow-level content.
pub struct DripHostedFlowType;

/// Policy for how the drip host is assigned
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DripHostPolicy {
    /// Strategy for initial host selection
    pub selection: HostSelectionStrategy,
    /// What to do when the current host drops
    pub failover: HostFailoverPolicy,
    /// How long to wait before declaring host dead
    pub host_timeout: Duration,
    /// Can the host change proactively (e.g., battery optimization)?
    pub allow_voluntary_handoff: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HostSelectionStrategy {
    /// First piece with can_host_drip that joins the ensemble
    FirstEligible,
    /// Prefer the piece with the best connectivity to all others
    BestConnected,
    /// Prefer a specific piece by device_id
    Preferred { device_id: Uuid },
    /// Use a scoring function based on capabilities
    Scored {
        /// Weight for: has UI, storage, battery, connectivity
        weights: HostScoreWeights,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HostScoreWeights {
    pub connectivity_weight: f32,     // how well connected to other pieces
    pub storage_weight: f32,          // storage capacity
    pub battery_weight: f32,          // battery level (if known)
    pub stability_weight: f32,        // how long online without interruption
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HostFailoverPolicy {
    /// Next eligible piece takes over immediately
    ImmediateFailover,
    /// Wait for timeout, then failover; queue edits in the meantime
    GracefulFailover { queue_edits: bool },
    /// All pieces maintain their own copy; merge when host returns
    OfflineMerge,
    /// No failover; flow pauses until host returns
    WaitForHost,
}
```

### 5.2 DripHostedFlow Implementation

```rust
// flow/types/drip_hosted.rs (continued)

pub struct DripHostedFlow<S: DocumentSchema> {
    /// Flow identity
    id: Uuid,
    schema: FlowSchema,

    /// The convergent document (CRDT state)
    document: Arc<RwLock<ConvergentDocument<S>>>,

    /// Host assignment policy
    policy: DripHostPolicy,

    /// Current host assignment
    current_host: Arc<RwLock<Option<Uuid>>>,

    /// Am I the current host?
    is_host: Arc<RwLock<bool>>,

    /// Ensemble manager reference (for topology awareness + host selection)
    ensemble: Arc<EnsembleManager>,

    /// Logical messaging layer (for sending/receiving flow data —
    /// hides multi-hop routing, flows never see BLE connections)
    messenger: Arc<dyn EnsembleMessenger>,

    /// Queued edits (when host is unavailable and policy allows queueing)
    edit_queue: Arc<Mutex<Vec<OpEnvelope>>>,

    /// Streams
    streams: HashMap<String, Box<dyn Stream>>,
}

impl<S: DocumentSchema> DripHostedFlow<S> {
    /// Evaluate host assignment based on current ensemble topology
    pub fn evaluate_host_assignment(&self) -> Option<Uuid> {
        let topology = self.ensemble.topology();
        let capsule = self.ensemble.capsule();

        match &self.policy.selection {
            HostSelectionStrategy::FirstEligible => {
                // Find first online piece that can host drips
                topology.online_pieces.iter()
                    .find(|(id, _)| {
                        capsule.pieces.iter()
                            .find(|p| p.device_id == **id)
                            .map(|p| p.capabilities.can_host_drip)
                            .unwrap_or(false)
                    })
                    .map(|(id, _)| *id)
            }
            HostSelectionStrategy::Scored { weights } => {
                // Score each eligible piece
                let mut best: Option<(Uuid, f32)> = None;
                for (id, presence) in &topology.online_pieces {
                    if let Some(piece) = capsule.get_piece(id) {
                        if !piece.capabilities.can_host_drip { continue; }
                        let score = self.score_piece(piece, presence, &topology, weights);
                        if best.is_none() || score > best.unwrap().1 {
                            best = Some((*id, score));
                        }
                    }
                }
                best.map(|(id, _)| id)
            }
            // ... other strategies
        }
    }

    /// Handle host dropout
    pub async fn handle_host_dropout(&self, dropped_host: Uuid) {
        match &self.policy.failover {
            HostFailoverPolicy::ImmediateFailover => {
                // Re-evaluate host assignment
                if let Some(new_host) = self.evaluate_host_assignment() {
                    self.assign_host(new_host).await;
                }
            }
            HostFailoverPolicy::GracefulFailover { queue_edits } => {
                if *queue_edits {
                    // Keep accepting edits into queue
                    // Start timeout timer
                    // After timeout: failover to new host, replay queue
                }
            }
            HostFailoverPolicy::OfflineMerge => {
                // Each piece keeps accepting edits locally
                // CRDT ensures convergence when pieces reconnect
                // No need for explicit failover
            }
            HostFailoverPolicy::WaitForHost => {
                // Pause the flow, notify UI
            }
        }
    }

    /// Become the host (start serving drip state)
    async fn become_host(&self) {
        *self.is_host.write().unwrap() = true;
        // Start accepting edits from the "edits" jet streams
        // Apply them to the convergent document
        // Broadcast updated state on the "state" drip
    }

    /// Relinquish hosting (hand off to another piece)
    async fn handoff_host(&self, new_host: Uuid) {
        // 1. Flush any pending edits
        // 2. Send full state to new host
        // 3. Update host_assignment drip
        // 4. Stop accepting direct edits
        *self.is_host.write().unwrap() = false;
    }
}
```

### 5.3 Accessory Memorize Role

The accessory can't host drips but can memorize flow state — storing operations and serving cached state when other pieces reconnect.

**Note on relaying**: Traffic routing through the accessory (e.g., forwarding edits from one piece to another) is handled transparently by the transport layer (EnsembleManager), not by the flow role. All pieces — including accessories — route traffic through themselves when they sit between two other pieces in the topology, like IP routers. The accessory needs no flow-level awareness to do this. Memorization, by contrast, is a flow role: the accessory must understand enough about the flow's data model to store and serve operations.

```rust
/// Memorization role for an accessory in a DripHostedFlow
pub struct AccessoryMemorizer {
    /// Local copy of the convergent document (for memorization)
    document: Arc<RwLock<ConvergentDocument<S>>>,
}

impl AccessoryMemorizer {
    /// Receive and store a state update from the host
    pub async fn memorize_state(&self, ops: Vec<OpEnvelope>) -> Result<(), FlowError>;

    /// If the host drops and this accessory has the most recent state,
    /// serve it to pieces that reconnect (read-only until a new host is assigned)
    pub async fn serve_cached_state(&self) -> Result<Vec<OpEnvelope>, FlowError>;
}
```

### 5.4 Flow Type Registration

Register the DripHostedFlow with the existing FlowRegistry:

```rust
fn register_drip_hosted_flow(registry: &mut FlowRegistry) {
    registry.register("drip_hosted", |config| {
        let policy: DripHostPolicy = serde_json::from_value(config.params.clone())
            .map_err(|e| FlowError::ConfigurationError(e.to_string()))?;

        // Create schema with required streams
        let schema = FlowSchema::new("DripHostedFlow")
            .with_stream(StreamSpec::drip("state"))
            .with_stream(StreamSpec::jet("edits", StreamCardinality::PerParty))
            .with_stream(StreamSpec::drip("host_assignment"));

        Ok(Box::new(DripHostedFlow::new(config, schema, policy)))
    });
}
```

### 5.5 Deliverables

- [ ] `DripHostPolicy`, `HostSelectionStrategy`, `HostFailoverPolicy` types
- [ ] `DripHostedFlow<S>` implementing the `Flow` trait
- [ ] Host evaluation logic (scoring based on capabilities + topology)
- [ ] Host failover logic (immediate, graceful, offline-merge, wait)
- [ ] `AccessoryMemorizer` for memorization (routing is transport-layer, not flow-level)
- [ ] Registration with `FlowRegistry`
- [ ] Unit tests for host selection strategies
- [ ] Unit tests for failover scenarios
- [ ] Integration test: 3-piece ensemble where host drops and failover occurs

---

## Phase 5.5: Giantt & Inventory Sync Demo

This is the first concrete payoff — syncing real app data between devices. Both Giantt and Inventory already use `ConvergentDocument` with full CRDT semantics, making them nearly ready to ride on the infrastructure built in Phases 0–5.

### 5.5.1 Wire Giantt ConvergentDocument Through DripHostedFlow

**Files**: `flow/types/giantt_sync.rs` (new), modifications to `ffi/giantt_flow.rs`

The existing `ConvergentDocument<GianttSchema>` already supports:
- `apply_local()` / `apply_remote()` for operations
- `operations_since(horizon)` for incremental sync
- `materialize()` for state

What's needed:
1. **Create a `GianttSyncFlow`** — a `DripHostedFlow<GianttSchema>` instance
2. **Bridge the existing FFI**: The current `ffi/giantt_flow.rs` manages `ConvergentDocument<GianttSchema>` directly via a global registry. Refactor so the document lives _inside_ a `DripHostedFlow`, and the FFI calls write operations into the flow's "edits" jet stream rather than directly into the document.
3. **Flow-aware sync**: When the ensemble manager connects to a peer, the flow's drip stream exchanges `operations_since()` automatically via the BLE connection.

```rust
/// Create the Giantt sync flow for a capsule
pub fn create_giantt_flow(
    capsule_id: Uuid,
    ensemble: Arc<EnsembleManager>,
    device_id: &str,
    data_dir: &Path,
) -> Result<DripHostedFlow<GianttSchema>, FlowError> {
    let policy = DripHostPolicy {
        selection: HostSelectionStrategy::FirstEligible,
        failover: HostFailoverPolicy::OfflineMerge, // CRDTs converge naturally
        host_timeout: Duration::from_secs(30),
        allow_voluntary_handoff: true,
    };

    // Load or create the ConvergentDocument from disk
    let doc = load_or_create_giantt_doc(device_id, data_dir)?;

    DripHostedFlow::new_with_document(
        Uuid::new_v4(),
        "giantt_sync".to_string(),
        doc,
        policy,
        ensemble,
    )
}
```

**Note on OfflineMerge**: Since both Giantt and Inventory use operation-based CRDTs with informed-remove semantics, the simplest failover policy is `OfflineMerge` — every piece keeps accepting edits locally, and CRDT convergence guarantees consistency when pieces reconnect. The drip host is less about being the single writer and more about being the piece that actively pushes state updates to others — a "sync coordinator" role.

### 5.5.2 Wire Inventory ConvergentDocument Through DripHostedFlow

Identical pattern to Giantt. The existing `ffi/inventory_flow.rs` manages `ConvergentDocument<InventorySchema>` — same refactor to route through a flow.

```rust
pub fn create_inventory_flow(
    capsule_id: Uuid,
    ensemble: Arc<EnsembleManager>,
    device_id: &str,
    data_dir: &Path,
) -> Result<DripHostedFlow<InventorySchema>, FlowError> {
    // Same pattern as Giantt
}
```

### 5.5.3 Sync Protocol via EnsembleMessenger

Flow sync uses the `EnsembleMessenger` (Phase 3.4) for all data exchange. Flows never touch BLE connections directly — they call `messenger.send_to(peer_id, FlowSync, payload)` and the messenger handles routing, whether the peer is one hop away or three. This means the same sync protocol works identically for directly connected pieces and multi-hop routed pieces.

When two pieces in the ensemble become reachable (directly or through intermediaries):

```
Piece A (host)                    Piece B
────────────                      ────────
1. Exchange horizons              1. Send my horizon
   messenger.send_to(B,              messenger.send_to(A,
     FlowSync, horizon_A)              FlowSync, horizon_B)

2. Compute ops B hasn't seen      2. Compute ops A hasn't seen
   ops = doc.operations_since(       ops = doc.operations_since(
           horizon_B)                        horizon_A)

3. Send ops to B                  3. Send ops to A
   messenger.send_to(B,              messenger.send_to(A,
     FlowSync, ops)                    FlowSync, ops)

4. Apply B's ops                  4. Apply A's ops
   doc.apply_remote(each)            doc.apply_remote(each)

5. Both now converged             5. Both now converged
```

All messages are wrapped in `RoutedEnvelope` by the messenger. If B is not directly connected to A but is reachable through piece C, the messenger routes through C transparently — A's flow code is identical whether B is direct or routed.

**Triggering sync**: The `EnsembleManager` notifies flows when a new piece becomes reachable (directly or indirectly via peer introduction). Flows subscribe to reachability changes and initiate horizon exchange when a new peer appears, regardless of how many hops away it is.

### 5.5.4 Demo Scenario

1. Mac has Giantt tasks and inventory items (from existing CLI/app usage)
2. Pair Android phone into the capsule (Phase 4)
3. On the Mac, Giantt and Inventory flows are created and attached to the capsule
4. Android phone discovers Mac via BLE, joins ensemble
5. Mac (as drip host) pushes all operations to Android phone
6. Android phone materializes the state → sees all tasks and inventory items
7. User edits a task on the Android phone → edit flows back to Mac
8. Both devices show consistent state

### 5.5.5 Deliverables

- [ ] `GianttSyncFlow` wrapping `DripHostedFlow<GianttSchema>`
- [ ] `InventorySyncFlow` wrapping `DripHostedFlow<InventorySchema>`
- [ ] Refactor `ffi/giantt_flow.rs` to route through flow (backward-compatible: local-only mode if no ensemble)
- [ ] Refactor `ffi/inventory_flow.rs` similarly
- [ ] Flow sync via `EnsembleMessenger` (not direct BLE connections)
- [ ] Horizon-based incremental sync (works over direct and multi-hop routes)
- [ ] Demo: edit Giantt task on Mac, see it appear on Android (and vice versa)
- [ ] Demo: edit inventory on one device, see it on the other

---

## Phase 6: Integration Testing & End-to-End Demo

### 6.1 Rust Integration Test: Full 3-Piece Scenario

**File**: `packages/soradyne_core/tests/three_piece_capsule.rs`

```rust
#[tokio::test]
async fn test_three_piece_capsule_lifecycle() {
    // 1. Create identities for Mac, Phone, Accessory
    // 2. Create SimBleNetwork with topology: Mac↔Accessory, Mac↔Phone
    //    (Phone and Accessory are NOT directly connected)
    // 3. Mac creates capsule
    // 4. Mac pairs with Accessory (simulated, in-process)
    // 5. Mac pairs with Phone (simulated, since this is a test)
    // 6. All three start ensemble discovery
    // 7. Verify: all three see each other in topology
    //    - Phone sees Accessory as Indirect { next_hop: Mac, hop_count: 1 }
    //    - Accessory sees Phone as Indirect { next_hop: Mac, hop_count: 1 }
    // 8. Create a DripHostedFlow (e.g., with inventory schema)
    // 9. Verify: Mac or Phone selected as drip host (not Accessory)
    // 10. Phone sends an edit via messenger.send_to(Mac, FlowSync, ops)
    //     → host applies → state propagates to all three (including
    //     Accessory, routed through Mac)
    // 11. Host goes offline → verify failover
    // 12. Accessory serves cached state to reconnecting piece
    // 13. Verify multi-hop: Phone sends directly to Accessory via
    //     messenger.send_to(Accessory, ...) — routed through Mac transparently
}
```

### 6.2 Mixed Real+Simulated BLE Test

**Manual test procedure** (cannot be fully automated):

1. Build soradyne_core with btleplug support
2. Run on Mac: create capsule, start ensemble
3. Create simulated accessory (in-process)
4. Run soradyne_demo on Android phone: join capsule via real BLE pairing
5. Verify on Mac: topology shows all three pieces
6. Verify on Android phone: topology shows Mac and Accessory
7. Create DripHostedFlow, verify host assignment
8. Test data flow: edit on Phone → Mac (host) → Accessory (memorize)
9. Kill Mac process → verify failover to Phone as new host
10. Restart Mac → verify it rejoins ensemble and syncs missed edits

### 6.3 Demo App Screens for Monitoring

Add monitoring screens to soradyne_demo:

- **EnsembleMonitorScreen**: Real-time view of which pieces are online, connection quality, topology graph
- **FlowMonitorScreen**: Shows active flows, current host, stream state
- **HostAssignmentLogScreen**: History of host changes with reasons

### 6.4 Deliverables

- [ ] Rust integration test for full lifecycle
- [ ] Manual test procedure documentation
- [ ] Demo app monitoring screens
- [ ] Performance benchmarks (advertisement encryption latency, CRDT sync throughput over BLE)

---

## Phase 7: Photo Flows & Album Port

This phase extends the working sync infrastructure to media, which is more demanding (larger payloads, progressive loading, different CRDT semantics).

### Instructive Example: Three Tiers of Image-Related Flows

Before detailing each sub-phase, it's worth laying out how images, composites, and albums relate as flows — because this is an instructive example of how the Rim protocol's flow/stream vocabulary handles a domain where the natural abstractions are **neither jets nor drips**.

**Key insight: not every stream is a jet or a drip.** The rim-protocol spec defines jets (continuous/event-driven, lossy, per-observer) and drips (convergent, consensus, authoritative) as _descriptive hints_, not an exhaustive taxonomy. A stream can have no category at all. An image is the clearest example: you query it for data at a resolution, and data comes back. It's not streaming continuously (jet), and there's no multi-party consensus to fuse (drip, in the base case). It's a **query-response stream** — a memorization-backed spatial data source that you read from.

**Three flow types, three different stream semantics:**

```
┌─────────────────────────────────────────────────────────────────┐
│  Album Flow (has a drip)                                        │
│  UUID: album-001                                                │
│                                                                 │
│  drip: convergent collection of composite references            │
│        [composite-A, composite-B, composite-C, ...]             │
│        ← this needs consensus: what's in the album, ordering    │
│                                                                 │
│  For each entry, the album reads from the composite flow,       │
│  picks out the image data, ignores the extra data (for now).    │
└──────────┬──────────────────────┬───────────────────────────────┘
           │                      │
           ▼                      ▼
┌─────────────────────┐  ┌─────────────────────┐
│ Image Composite Flow│  │ Image Composite Flow│
│ UUID: composite-A   │  │ UUID: composite-B   │
│ (has a drip)        │  │                     │
│                     │  │ (same structure)    │
│ drip: convergent    │  └─────────────────────┘
│ sequence of compo-  │
│ sition ops (for     │
│ collaborative edits)│
│                     │
│ Returns:            │
│  - image data       │
│  - extra data       │
│    (SVGs, vectors,  │
│     annotations —   │
│     dummy struct    │
│     for now)        │
│                     │
│ Current impl: just  │
│ one "edit" = genesis │
│ load from an image  │
│ flow at a resolution│
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Image Flow          │
│ UUID: image-001     │
│                     │
│ Query-response      │
│ stream (neither     │
│ jet nor drip).      │
│                     │
│ Query: resolution   │
│  (small/preview/    │
│   large/full)       │
│                     │
│ Returns: image data │
│ at requested res.   │
│                     │
│ Spatially aware.    │
│ Memorization-backed.│
└─────────────────────┘
```

**Why this layering matters:**
- An **image flow** is the raw spatial data. It knows about resolution but nothing about editing, composition, or collections. Different pieces in the ensemble can each memorize the image; a request traverses fittings that might do format conversion (e.g., if the requesting device lacks ffmpeg).
- An **image composite flow** combines data from image flows (and eventually other sources — vector overlays, layer transforms). It has a **drip stream** for its composition operations: multiple parties can contribute edits (transforms, layers, annotations) that need convergence. It's also memorized, so it has a persistent history of those operations. For now this is a stub: one operation (genesis load from an image flow at a resolution), returning image data plus a dummy extra-data struct.
- An **album flow** is a collection with a drip. The drip needs consensus because multiple people might add/remove/reorder entries. Each entry references a composite flow by UUID. The album reads from the composite and picks out the image part (ignoring the extra data for now — bookmarking that the composite returns more than just pixels).

### 7.1 Image Flow (Query-Response Stream, Neither Jet Nor Drip)

An image flow stores image data and serves it in response to resolution-parameterized queries. It is spatially aware (images are 2D data) and memorization-backed (the data persists across sessions).

```rust
/// Image flow: spatial data source for a single image
///
/// Streams:
///   - "image_data" (no category — query-response): Returns image data at
///     a requested resolution. The resolution parameter is part of the query.
///   - "metadata" (no category — read-once): Image dimensions, format, size, etc.
///
/// Roles:
///   - Memorizer: Stores the image data (block storage / dissolution)
///   - (Future) Converter: Provides format conversion fittings for devices
///     that lack specific codec support
///
/// Query parameters:
///   - resolution: Discrete { Small, Preview, Large, Full }
///     (keeps computation simple for now; continuous resolution is future work)
pub struct ImageFlowType;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ImageResolution {
    Small,    // ~150px, suitable for thumbnails / grid views
    Preview,  // ~600px, suitable for preview / list views
    Large,    // ~1200px, suitable for detail views
    Full,     // Original resolution
}

/// What an image flow returns for a query
#[derive(Clone, Debug)]
pub struct ImageQueryResult {
    /// The image data at the requested resolution
    pub data: Vec<u8>,
    /// Actual resolution delivered (may differ from requested if not available)
    pub actual_resolution: ImageResolution,
    /// Image format (png, jpeg, webp, etc.)
    pub format: String,
    /// Original dimensions
    pub original_width: u32,
    pub original_height: u32,
}
```

**Implementation**:
- Image data lives in block storage (existing dissolution system)
- Each resolution tier can be pre-computed and cached, or computed on demand
- The stream's `read()` is parameterized by resolution (encoded in the query)
- Multiple pieces can memorize the same image; the flow's sync is just "do you have this block?"
- Future: fittings for format conversion (e.g., a piece without ffmpeg requests JPEG; a piece that has the raw HEIC serves a fitting that converts)

### 7.2 Image Composite Flow (Memorized Composition, Stub For Now)

An image composite flow produces a composed result from one or more input sources. It has a memorized history of composition operations — but the history is incidental to memorization, not the defining characteristic. The flow's identity is "this is a composite image."

```rust
/// Image composite flow: collective composition from image sources + transforms
///
/// Streams:
///   - "rendered" (no category — query-response): Returns the composited
///     image data + extra data at a requested resolution
///   - "operations" (drip, singleton): The memorized sequence of composition
///     operations. This is a drip because multiple parties can contribute
///     edits that need convergence (future: collaborative editing).
///     For now, typically just one genesis operation.
///
/// Returns two things:
///   - Image data (pixels)
///   - Extra data (a struct that will eventually hold SVGs, vector annotations,
///     layer metadata, etc. — currently a placeholder/dummy)
///
/// Current implementation (stub):
///   A single composition operation: "load image from flow UUID at resolution X."
///   The composite just passes through the image data from the source flow.
///   Extra data is a dummy empty struct.
pub struct ImageCompositeFlowType;

/// A composition operation (will grow over time)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CompositeOp {
    /// Genesis: load base image from a source image flow
    LoadFromImageFlow {
        source_flow_id: Uuid,
        resolution: ImageResolution,
    },
    // Future operations (stubs, not implemented yet):
    // ApplyTransform { matrix: TransformMatrix, start: Vec2, end: Vec2 },
    // OverlayImage { source_flow_id: Uuid, position: Vec2, opacity: f32 },
    // AddVectorLayer { svg_data: String },
    // Crop { rect: CropRect },
    // Rotate { degrees: u16 },
}

/// What a composite flow returns
#[derive(Clone, Debug)]
pub struct CompositeQueryResult {
    /// The composed image data
    pub image: ImageQueryResult,
    /// Extra data (annotations, vectors, layer info — dummy for now)
    pub extra: CompositeExtraData,
}

/// Placeholder for non-image data that composites will eventually carry
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CompositeExtraData {
    // Future: svg_layers, vector_annotations, text_overlays, etc.
    // For now, this is intentionally empty — it exists to bookmark that
    // the composite returns more than just pixels.
}
```

**Current implementation**: The composite flow stores a single `LoadFromImageFlow` operation. When queried, it reads from the referenced image flow at the requested resolution and returns the result plus an empty `CompositeExtraData`. This is a minimal stub that establishes the three-tier architecture without requiring any actual compositing logic.

**Future**: Multiple `CompositeOp` entries compose a rendering pipeline. The operations stream becomes a drip when collaborative editing is added (multiple parties contributing transforms/layers). JWST+Hubble overlay is exactly the kind of thing this enables — two image flows as sources, a composite flow that layers them with transforms.

### 7.3 Album Flow (Drip — Convergent Collection of Composites)

An album flow is a collection whose membership and ordering need consensus. This is a genuine drip: multiple parties can add, remove, or reorder entries, and the result must converge.

Each album entry references an image composite flow by UUID. The album reads from the composite and picks out the image data (ignoring the extra data for now).

**Port from album/ module**: The existing album module has its own CRDT (`LogCrdt` + `EditOp` + `MediaAlbum`). This will be ported to `ConvergentDocument<AlbumSchema>`, gaining horizon-based informed-remove semantics and unified sync.

```rust
// convergent/album.rs (new)

/// Schema for album documents.
/// An album is a collection of references to image composite flows.
#[derive(Clone)]
pub struct AlbumSchema;

impl DocumentSchema for AlbumSchema {
    type State = AlbumState;

    fn item_types(&self) -> HashSet<String> {
        HashSet::from(["AlbumEntry".into()])
    }

    fn item_type_spec(&self, type_name: &str) -> Option<Box<dyn ItemTypeSpec>> {
        match type_name {
            "AlbumEntry" => Some(Box::new(AlbumEntrySpec)),
            _ => None,
        }
    }
}

/// An album entry references a composite flow and carries album-specific metadata
struct AlbumEntrySpec;
impl ItemTypeSpec for AlbumEntrySpec {
    fn type_name(&self) -> &str { "AlbumEntry" }
    fn fields(&self) -> Vec<FieldSpec> {
        vec![
            FieldSpec::required("composite_flow_id"),  // UUID of the image composite flow
            FieldSpec::optional("position"),            // Ordering within the album
            FieldSpec::optional("caption"),
            FieldSpec::optional("added_by"),            // Device that added this entry
            FieldSpec::optional("added_at"),            // Timestamp
        ]
    }
    fn sets(&self) -> Vec<SetSpec> {
        vec![
            SetSpec::new("tags"),
            SetSpec::new("comments"),
            SetSpec::new("reactions"),
        ]
    }
}
```

**What the album does with a composite**: When rendering an album view, the album reads each entry's `composite_flow_id`, queries that composite flow for the image at the appropriate resolution (e.g., `Small` for grid view, `Large` for detail), and displays the image part of the `CompositeQueryResult`. The `extra` field is ignored for now — this bookmarks that the album could eventually display annotations, vectors, etc.

### 7.4 Mapping Existing Album Operations

The existing `EditOp` variants map to the new architecture:

| Existing `EditOp` | New location | Mechanism |
|---|---|---|
| `set_media` | Image flow creation + album entry | Create an image flow, then a composite flow referencing it, then add album entry |
| `rotate` / `crop` | Composite flow operation | Future `CompositeOp::Rotate` / `CompositeOp::Crop` (stub for now) |
| `add_comment` | Album entry `AddToSet("comments", ...)` | ConvergentDocument operation on the album |
| `add_reaction` | Album entry `AddToSet("reactions", ...)` | ConvergentDocument operation on the album |
| `delete` | Album entry `RemoveItem` | Informed-remove (better than tombstone) |
| `share_with` | Album entry `SetField("shared_with", ...)` | ConvergentDocument operation |

### 7.5 Migration Path

For existing album data (stored via `BlockManager` + `MediaAlbum`):
1. For each existing media item:
   a. Create an image flow (UUID), referencing the existing block storage data
   b. Create an image composite flow (UUID) with a single `LoadFromImageFlow` operation
2. Create an album flow with a `ConvergentDocument<AlbumSchema>`
3. For each media item, add an `AlbumEntry` referencing the composite flow UUID
4. Migrate comments, reactions, tags to the album entry's sets
5. Going forward, all edits go through the flow architecture

### 7.6 Deliverables

- [ ] `ImageFlowType` with resolution-parameterized query-response stream
- [ ] `ImageCompositeFlowType` with stub implementation (genesis load + dummy extra data)
- [ ] `CompositeOp` enum with `LoadFromImageFlow` variant
- [ ] `CompositeExtraData` placeholder struct
- [ ] `AlbumSchema` + `AlbumEntrySpec` for `ConvergentDocument`
- [ ] `AlbumSyncFlow` wrapping `DripHostedFlow<AlbumSchema>`
- [ ] Migration tool: existing `MediaAlbum` → image flows + composite flows + album flow
- [ ] FFI updates for image/composite/album operations
- [ ] Demo: add image on Android → image flow + composite flow created → album entry syncs to Mac

---

## Appendix A: Crate/Module Layout Decisions

### New modules within `soradyne_core`:

```
packages/soradyne_core/src/
├── ble/                          # NEW: BLE transport layer
│   ├── mod.rs                    # Module exports
│   ├── transport.rs              # BleCentral, BlePeripheral, BleConnection traits
│   ├── simulated.rs              # SimBleNetwork, SimBleDevice
│   ├── real_ble.rs               # btleplug-based real BLE
│   ├── encrypted_adv.rs          # Advertisement encryption/decryption
│   ├── gatt.rs                   # GATT service definitions
│   └── error.rs                  # BLE-specific errors
├── topology/                     # NEW: Device topology
│   ├── mod.rs                    # Module exports
│   ├── capsule.rs                # Capsule, PieceRecord, PieceCapabilities
│   ├── capsule_store.rs          # Capsule add-only set with gossip merge
│   ├── capsule_store.rs          # CapsuleStore persistence
│   ├── ensemble.rs               # EnsembleManager, EnsembleTopology
│   ├── messenger.rs              # EnsembleMessenger trait, TopologyMessenger, RoutedEnvelope forwarding
│   ├── pairing.rs                # PairingEngine, PairingVerifier
│   └── error.rs                  # Topology-specific errors
├── flow/
│   ├── types/                    # NEW: Flow type implementations
│   │   ├── mod.rs
│   │   └── drip_hosted.rs        # DripHostedFlow, DripHostPolicy
│   └── ... (existing)
├── identity/                     # FILLED IN (currently stubs)
│   ├── mod.rs
│   ├── keys.rs                   # DeviceIdentity
│   ├── capsule_keys.rs           # CapsuleKeyBundle (NEW)
│   └── auth.rs                   # DeviceAuthenticator
├── ffi/
│   ├── topology_bridge.rs        # NEW: Capsule/ensemble FFI
│   ├── pairing_bridge.rs         # NEW: Pairing FFI
│   └── ... (existing)
└── ... (existing modules unchanged)
```

### New Flutter files:

```
apps/soradyne_demo/flutter_app/lib/
├── screens/
│   ├── capsule/                  # NEW
│   │   ├── capsule_list_screen.dart
│   │   ├── capsule_detail_screen.dart
│   │   └── ensemble_monitor_screen.dart
│   ├── pairing/                  # NEW
│   │   ├── pairing_invite_screen.dart
│   │   ├── pairing_join_screen.dart
│   │   ├── pin_verification_screen.dart
│   │   └── pairing_complete_screen.dart
│   └── ... (existing)
├── services/
│   ├── capsule_service.dart      # NEW (FFI to Rust capsule/ensemble)
│   └── ... (existing)
└── ffi/
    ├── topology_bindings.dart    # NEW
    ├── pairing_bindings.dart     # NEW
    └── ... (existing)
```

---

## Appendix B: Dependency Inventory

### New Rust dependencies (Cargo.toml additions)

```toml
# Cryptographic identity
ed25519-dalek = { version = "2", features = ["serde"] }
x25519-dalek = { version = "2", features = ["static_secrets"] }
curve25519-dalek = "4"
zeroize = { version = "1", features = ["derive"] }

# BLE transport (real)
btleplug = { version = "0.11", optional = true }

# Serialization (already have serde/serde_json; may want CBOR for compact BLE payloads)
ciborium = "0.2"  # CBOR serialization (compact, good for BLE)

# Time (already have chrono, but may want tokio::time for intervals)
# (already available via tokio features)
```

**New Cargo features**:
```toml
[features]
default = ["video-thumbnails"]
ble-real = ["btleplug"]        # Real BLE support (requires platform BLE stack)
ble-simulated = []              # Always available (no extra deps)
```

### New Flutter dependencies (pubspec.yaml additions)

```yaml
# UI components
qr_flutter: ^4.0.0              # Future: QR code pairing alternative
```

**Note**: No Flutter BLE plugin is needed. BLE is owned entirely by Rust (btleplug) and runs within the `libsoradyne.so` loaded by the Flutter app. The Flutter side only calls FFI for UI-level actions.

---

## Appendix C: Open Questions & Future Context

### C.1 btleplug Peripheral Role on macOS

btleplug primarily supports the BLE central role (scanning, connecting). The peripheral role (advertising, accepting connections) on macOS requires CoreBluetooth's `CBPeripheralManager`. Options:
1. Use `core-bluetooth` crate for peripheral role + btleplug for central
2. Use a combined approach via raw CoreBluetooth FFI
3. Start with the Mac as central-only (phone advertises, Mac scans)

**Recommendation**: Start with Mac as central, Phone as peripheral. This is the most common BLE pattern and btleplug handles it well. Add Mac peripheral capability later if needed. On Android, btleplug's JNI layer can access `BluetoothLeAdvertiser` for the peripheral role, so the phone can advertise natively from Rust.

### C.2 BLE Data Transfer MTU and Throughput

With BLE 5.0 targeted, the default ATT MTU is still 23 bytes but can be negotiated up to 512 bytes (btleplug handles this). BLE 5.0 also offers 2 Mbps PHY throughput (vs 1 Mbps in 4.x). For CRDT operations that can be large, we still need:
- MTU negotiation to 512 bytes (btleplug handles this automatically)
- Chunking/reassembly layer on top of GATT writes for payloads exceeding MTU
- Flow control for large sync batches

**Recommendation**: Negotiate MTU to maximum on connection. Implement a simple chunked transfer protocol over the GATT characteristic for payloads exceeding MTU. Header: 2 bytes (chunk index, total chunks), remaining bytes are payload. With 512-byte MTU, most individual CRDT operations fit in a single write — chunking is mainly needed for initial bulk sync.

### C.3 Capsule Retirement vs. Piece Removal

The Rim spec says "retired whole rather than shrunk." This means if a device is compromised, you retire the entire capsule and create a new one with the remaining devices. This is clean but operationally heavy. For the POC:
- Implement single-piece removal as a convenience operation
- Also implement full capsule retirement
- Document that single-piece removal is weaker security

### C.4 Android App Architecture

The Android phone runs:
- **Flutter UI** (soradyne_demo app running on Android) — for capsule management, pairing, flow state display
- **Rust core** (soradyne_core compiled for Android arm64 via NDK + FFI — already documented in CLAUDE.md) — owns BLE, sync, CRDT, everything

BLE is handled entirely within Rust via btleplug's Android/JNI support. The Flutter app loads `libsoradyne.so` and the Rust runtime within it manages BLE scanning, advertising, connections, and protocol logic. Flutter never touches the radio:

```
Flutter UI (Dart)
    ↕ FFI (create capsule, start pairing, get flow state, etc.)
Rust core (btleplug BLE, CRDT sync, topology, pairing)
    ↕ JNI (btleplug ↔ Android BLE stack)
Android BluetoothAdapter
```

This is architecturally clean: adding a different frontend (Tauri, Unity) requires zero BLE work — only new FFI calls for UI actions. Sync works even without any UI running.

Android cross-compilation is already set up (see CLAUDE.md: `cargo ndk -t arm64-v8a build --release --no-default-features`). The `.so` goes into `soradyne_flutter/android/src/main/jniLibs/arm64-v8a/`.

### C.5 Simulated Accessory Lifecycle

The simulated accessory runs in the same process as the Mac's soradyne_core. Options:
1. **Same thread, async tasks**: Accessory is a set of tokio tasks within the Mac's runtime
2. **Separate thread**: Accessory runs on its own thread with its own runtime
3. **Separate process**: Accessory is a standalone binary communicating via simulated BLE (most realistic, most complex)

**Recommendation**: Option 1 for the POC. The simulated BLE network already provides logical separation. The accessory's "device identity" is separate even though it shares a process.

### C.6 Desktop Flow Viewer (Tauri) as Modularity Proof

A Tauri desktop app that inspects and visualizes live flow state would serve as a **proof of modularity** — demonstrating that the Soradyne Rust core works with frontends other than Flutter. Since btleplug owns BLE and the entire sync stack runs in Rust, a Tauri app would:

- Link directly against `soradyne_core` (no FFI needed — same language)
- Get BLE discovery, ensemble topology, and flow sync for free
- Provide a developer-facing view: ConvergentDocument operation logs, ensemble topology graph, drip host assignment history, connection quality metrics
- Prove that the architecture is frontend-agnostic

This is not a priority for the current plan but is a natural follow-on after Phase 6 (integration testing). The investment would be small since the Rust core does all the heavy lifting.

### C.7 VR Headset Support (Quest 2 / Unity / C#)

The Meta Quest 2 runs Android under the hood, which means the same `libsoradyne.so` compiled for Android arm64 works on it. From Unity (C#), the Rust library is loaded via `P/Invoke` (the standard .NET mechanism for calling native libraries) — the same `extern "C"` FFI functions used by Flutter/Dart work identically from C#.

```
Unity (C#)
    ↕ P/Invoke (same extern "C" FFI as Flutter uses)
libsoradyne.so (Rust core — btleplug BLE, CRDT sync, topology)
    ↕ JNI (btleplug ↔ Android BLE stack on Quest 2)
Android BluetoothAdapter (Quest 2's underlying Android OS)
```

No C# reimplementation of the sync protocol is needed. The Rust core provides BLE, CRDT, ensemble management, and flow sync; Unity provides the VR-specific UI and spatial rendering. This is the same architecture as the Flutter app, just with a different frontend.

This is bookmarked for future exploration — the key design implication is that the FFI surface area should remain `extern "C"` compatible (no Dart-specific assumptions) to support this path.

### C.8 Key Rotation and Epoch Management

The `CapsuleKeyBundle` includes an `epoch` for future key rotation. For the POC:
- Single epoch (no rotation)
- Document the rotation mechanism design
- Implement rotation in a later phase

### C.9 Flow Configuration Distribution

The `DripHostedFlow` needs all pieces to know about it. Currently `FlowConfigStorage` is in-memory. For the POC:
- Flow configurations are stored alongside the capsule as a simple list (add-only, same gossip pattern as the capsule's piece set)
- When a new flow is created, the creating piece adds a `FlowConfig` entry to the capsule's flow list
- Other pieces learn about flows through capsule gossip at connection time
- This keeps flow discovery at the same level as capsule discovery — pre-flow infrastructure, no bootstrapping issue

---

## Estimated Session Breakdown

This is a rough guide for how many Claude Code sessions each phase might take, assuming focused implementation work per session:

| Phase | Sessions | Dependencies | Demo payoff |
|-------|----------|-------------|-------------|
| 0: Crypto Identity | 1–2 | None | |
| 1: BLE Transport | 3–5 | Phase 0 | |
| 2: Device Topology | 2–3 | Phase 0 | |
| 3: Ensemble Discovery | 3–4 | Phases 1, 2 | |
| 4: Pairing UX | 2–3 | Phases 1, 2, 3 | |
| 5: Drip Host Flow | 2–3 | Phases 2, 3 | |
| 5.5: Giantt & Inventory Sync | 2–3 | Phases 3, 5 | **Giantt + Inventory synced between Mac & Android** |
| 6: Integration | 2–3 | All above | |
| 7: Photo Flows & Album Port | 3–5 | Phases 5.5, 6 | **Photo sharing + album sync** |
| **Total** | **~20–31** | | |

Phases 1 and 2 can partially overlap since BLE transport and topology data structures are independent (though ensemble discovery needs both). Phase 5.5 is the first major payoff — after that, real data syncs between real devices.

---

## Summary

This plan takes the codebase from its current state (in-memory flows, no device topology, no BLE, stub identity) to a working 3-piece capsule (2020 MacBook, Android phone, simulated accessory) with:

- **Cryptographic device identity** (Ed25519 + X25519) with capsule shared keys
- **BLE transport** (simulated + real via btleplug on all hosted platforms, esp-idf-hal on ESP32-S3) with encrypted advertisements — BLE owned entirely by Rust, frontend-agnostic
- **Capsule persistence** as a simple add-only set with gossip merge (pre-flow infrastructure, no CRDT engine needed, ESP32-compatible)
- **Ensemble discovery** with topology sync, peer-introduction protocol, and `EnsembleMessenger` for logical multi-hop routing (flows address peers by UUID, routing is invisible)
- **Signal-inspired pairing UX** with ECDH + PIN verification in the demo app
- **DripHostedFlow** type with policy-based host assignment and failover
- **Accessory support** for memorization role without hosting capability (routing is transport-layer, handled by all pieces)
- **Giantt & Inventory sync** as the first concrete demo (both already use ConvergentDocument, minimal wiring needed)
- **Photo album port** from the legacy `LogCrdt`/`EditOp` system to `ConvergentDocument<AlbumSchema>`, enabling album sync through the same flow infrastructure

The plan builds incrementally on the existing architecture patterns (traits, ConvergentDocument, Flow/Stream, FFI bridge) and introduces no unnecessary abstractions. iPad/iOS support is bookmarked for after the Android path is proven.

