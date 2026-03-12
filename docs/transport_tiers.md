# rim Transport Tiers

How capsule pieces discover and communicate with each other across
different network conditions, from in-process simulation through
global internet fallback.

All tiers produce `Box<dyn BleConnection>` and feed into the same
`EnsembleManager` / `TopologyMessenger` / Noise IKpsk2 session layer.
A single device can have connections from multiple tiers simultaneously.

---

## Tier 1: In-Process (SimBLE + LAN TCP)

**When**: Tests, local development, same-machine demos.

Two variants:

### 1a: SimBleNetwork (existing)

Pure in-memory channels. No real I/O. Deterministic with `start_paused`
time. Used by all unit and integration tests today.

### 1b: LanBleNetwork (TCP on localhost)

Real TCP sockets on `127.0.0.1` with an in-process service registry
standing in for discovery. Exercises real async I/O, framing, and
connection lifecycle while keeping discovery trivial.

The in-process registry mimics mDNS semantics: devices register
(address, encrypted_adv_payload) and scanners query the registry.
This means the code path through `EnsembleManager` — `handle_advertisement`,
`try_decrypt_advertisement`, `should_initiate_connection`, Noise handshake —
is identical to production.

**Implementation**: `ble/lan_transport.rs` with `LanBleNetwork` + `LanBleDevice`.

---

## Tier 2: LAN (mDNS/DNS-SD + TCP)

**When**: Devices on the same WiFi / Ethernet / local network.

Uses multicast DNS (RFC 6762) with DNS-SD (RFC 6763) for zero-configuration
service discovery. No server, no static IPs, no infrastructure.

### How BLE operations map

| BLE operation | LAN implementation |
|---|---|
| `start_advertising(data)` | Register `_rim._tcp.local` service via mDNS; encrypted adv payload goes in TXT record; TCP listener on random port |
| `start_scan()` | Browse for `_rim._tcp.local` services |
| `advertisements()` | mDNS discovery events → `BleAdvertisement { data: txt_record, source_address: BleAddress::Tcp(resolved_ip:port) }` |
| `connect(address)` | TCP connect to resolved IP:port |
| `accept()` | TCP accept on advertised port |
| `update_advertisement(data)` | Update mDNS TXT record |
| `stop_advertising()` | Deregister mDNS service |

The encrypted advertisement payload (capsule hint + encrypted piece hint +
topology hash) goes into the mDNS TXT record byte-for-byte. An eavesdropper
on the LAN sees the same opaque bytes as they would from a BLE advertisement.
`try_decrypt_advertisement` works unchanged.

IP addresses don't need to be static — mDNS re-resolves on every browse cycle.
DHCP lease changes are reflected in the next mDNS announcement.

**Dependency**: `mdns-sd` crate (pure Rust, async-compatible).

**Implementation**: `ble/mdns_transport.rs` with `MdnsBleDevice` implementing
both `BleCentral` and `BlePeripheral`, reusing `TcpConnection` for data transport.

---

## Tier 3a: WAN via Mesh VPN Overlay

**When**: User has Tailscale, Nebula, ZeroTier, Headscale, or similar.

A pluggable option, not a dependency. The mesh VPN gives each device a
stable virtual IP on a shared virtual network. Discovery can use:

- Static virtual IPs exchanged during pairing (they don't change)
- An mDNS reflector/proxy running on the virtual network
- Direct TCP connections to known virtual IPs

This gives Tier 2 semantics over WAN with no rim-specific infrastructure.
The tradeoff is that every rim user must install and configure a VPN tool.

**No rim code changes needed** — Tier 2's `MdnsBleDevice` (or even direct
TCP connections via `add_direct_connection`) works over the virtual network.
The only rim-side work is documentation and optional helper scripts for
common VPN setups.

---

## Tier 3b: WAN Native (STUN + Rendezvous + Hole Punching)

**When**: Devices on different networks (cell data, different WiFi,
different continents), no VPN, no shared LAN.

rim's own WAN connectivity with no external dependencies beyond a
lightweight rendezvous endpoint. Three components:

### 1. STUN (Session Traversal Utilities for NAT)

A STUN server reflects your public IP:port back to you. It's stateless,
trivial to run, and many are freely available (Google, Cloudflare, etc.).

Device boots → STUN query → learns `public_ip:port`.

### 2. Rendezvous / Signaling

A minimal server where authenticated devices publish presence info and
query for peers. Handles **zero data** — only signed presence announcements.

The server doesn't need to be trusted because:
- Devices sign their presence with their Ed25519 key (from `DeviceIdentity`)
- Capsule members verify against `PieceRecord.verifying_key` from pairing
- An attacker controlling the server can't forge announcements
- Presence blobs are encrypted to capsule members via the capsule key

A presence announcement looks like:
```
{
  capsule_hint: [u8; 4],           // cleartext, for routing
  encrypted_presence: Vec<u8>,     // encrypted to capsule key
  signature: [u8; 64],            // Ed25519 signature over the above
}
```

Inside the encrypted blob:
```
{
  device_id: Uuid,
  public_addrs: Vec<SocketAddr>,   // from STUN
  timestamp: u64,
  nonce: [u8; 16],                 // replay protection
}
```

The rendezvous server could be:
- A serverless function (Lambda / Cloudflare Worker) + KV store
- DNS TXT records under a rim-controlled domain
- A WebSocket endpoint for real-time presence updates
- A simple REST API with TTL-based expiry

### 3. NAT Traversal (UDP Hole Punching)

Once two devices know each other's public address (from rendezvous):

1. Both send UDP packets to each other's public IP:port simultaneously
2. The outbound packet creates a NAT mapping; the inbound packet from
   the peer matches it → bidirectional UDP flow established
3. Upgrade to reliable transport (QUIC, or TCP over the punched hole)
4. Noise IKpsk2 handshake, same as every other tier

This works for ~85% of NAT configurations (full-cone, restricted-cone,
port-restricted-cone). Symmetric NATs require the Tier 3c fallback.

### Flow

```
Device A                    Rendezvous                    Device B
   |                            |                            |
   |-- STUN query ------------->|                            |
   |<-- your public IP:port ----|                            |
   |                            |                            |
   |-- publish(signed presence)->|                           |
   |                            |<-- query(capsule_hint) --- |
   |                            |-- encrypted presence ----->|
   |                            |                            |
   |<------------- UDP hole punch -------------------------->|
   |                            |                            |
   |<------------- Noise IKpsk2 handshake ------------------>|
   |                            |                            |
   |<------------- encrypted transport --------------------->|
```

---

## Tier 3c: Global Relay Fallback

**When**: NAT traversal fails (symmetric NATs, corporate firewalls,
carrier-grade NAT). The final recourse.

A server at a globally known URL (e.g., `relay.rim.example`) that
relays encrypted traffic between devices that cannot establish a
direct connection.

### Properties

- **Zero trust**: The relay sees only Noise-encrypted ciphertext. It
  cannot read, modify, or forge messages. It's a dumb pipe.
- **Authenticated routing**: Devices authenticate to the relay using
  their Ed25519 device key. The relay routes based on capsule_hint +
  piece_hint, same as BLE advertisement routing.
- **Fallback only**: Devices always attempt direct connection first
  (Tier 2 → 3b hole punch). Relay is used only when direct fails.
- **Minimal state**: The relay holds only active sessions and routing
  tables. No user data, no message history, no accounts.

### How it works

1. Device connects to relay via WebSocket/QUIC at `relay.rim.example`
2. Authenticates with Ed25519 signature over a server-provided challenge
3. Registers its capsule_hint + piece_hint
4. Relay matches peers by capsule_hint and forwards encrypted bytes
5. Noise IKpsk2 handshake runs *through* the relay (relay can't decrypt)
6. Encrypted transport messages flow through the relay

The relay implements `BleConnection` — from `TopologyMessenger`'s
perspective, it's just another connection. The Noise session provides
end-to-end encryption regardless of how many relays are in the path
(same principle as the multi-hop relay tests already passing).

### Combining with rendezvous

The relay server and rendezvous server can be the same endpoint. A
device connects, publishes its presence, and if hole punching fails
within a timeout, the connection to the relay server itself becomes
the transport path.

---

## What rim already has for all of this

The authentication and encryption infrastructure is complete:

- **Mutual authentication**: Ed25519 keys exchanged during pairing
  (`DeviceIdentity`, `PieceRecord.verifying_key`)
- **Per-session encryption**: Noise IKpsk2, transport-agnostic
  (`SecureBleConnection` wraps any `BleConnection`)
- **Capsule membership as trust group**: `CapsuleKeyBundle` for
  PSK derivation, encrypted advertisements
- **Signed messages**: `DeviceIdentity.sign()` / `verify()`
- **Multi-hop relay**: `TopologyMessenger` already forwards through
  intermediate devices, each link independently encrypted

The missing pieces are discovery (mDNS for Tier 2, rendezvous for 3b)
and NAT traversal (STUN + hole punching for 3b, relay for 3c). The
transport itself (TCP connections) and everything above it (session
encryption, topology management, flow sync) are already tier-agnostic.

---

## Implementation Order

1. **Tier 1b**: `LanBleNetwork` — real TCP + in-process registry. Tests.
2. **Tier 2**: `MdnsBleDevice` — mDNS discovery + TCP transport.
3. **Tier 3b**: Rendezvous service + STUN + UDP hole punching.
4. **Tier 3c**: Relay server at a known URL. Fallback for failed hole punch.
5. **Tier 3a**: Documentation + helper scripts for VPN overlay users.
