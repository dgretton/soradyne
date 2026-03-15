//! mdns_sync_peer — CLI tool for testing mDNS-discovered CRDT sync between machines.
//!
//! Usage:
//!   # On machine A (first time — generates a capsule and prints invite code):
//!   cargo run --example mdns_sync_peer --no-default-features -- init
//!
//!   # On machine B (paste the invite code from machine A):
//!   cargo run --example mdns_sync_peer --no-default-features -- join <INVITE_CODE>
//!
//!   # On machine A (paste the response code from machine B):
//!   cargo run --example mdns_sync_peer --no-default-features -- accept <RESPONSE_CODE>
//!
//!   # Repeat join/accept for machine C, D, etc.
//!
//!   # On any machine (after init/join — starts syncing):
//!   cargo run --example mdns_sync_peer --no-default-features -- --port 7117
//!
//! The REPL supports: add <desc>, list, peers, quit
//!
//! ## Discovery
//!
//! Two discovery methods run in parallel:
//!
//! 1. **mDNS** — zero-config LAN discovery (works on same subnet)
//! 2. **Direct TCP** — for cross-network sync (e.g. Tailscale), configured via
//!    `~/.rim/mdns_sync_test/peers_addresses.json`:
//!    ```json
//!    {
//!      "<peer-uuid>": "100.105.222.128:7117",
//!      "<peer-uuid>": "100.98.48.124:7117"
//!    }
//!    ```
//!    Use `--port <PORT>` to set the listen port (default 7117).
//!
//! ## On key sharing
//!
//! The `init` / `join` subcommands exist ONLY for testing this transport layer.
//! They are NOT how rim works in production. The real flow is:
//!
//!   1. Two devices pair over Bluetooth using the PairingEngine
//!      (X25519 ECDH → PIN confirmation → encrypted CapsuleKeyBundle transfer)
//!   2. Once a capsule has ≥2 pieces, a new device can be invited via a
//!      "sign in with rim" flow relayed through an existing piece
//!
//! The copy-paste invite code here is a one-off test shim. It bundles the
//! capsule keys + device identity so two machines can bootstrap a capsule
//! without Bluetooth. This code is intentionally NOT factored into the
//! library — it lives entirely in this example file.

use std::collections::HashMap;
use std::io::{self, BufRead, Write as _};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

/// Encode bytes as hex string (no external dep needed).
fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Decode hex string to bytes.
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err("odd-length hex string".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use soradyne::ble::lan_transport::LanConnection;
use soradyne::ble::mdns_transport::MdnsTransport;
use soradyne::ble::transport::{BleAddress, BleCentral, BleConnection, BlePeripheral};
use soradyne::convergent::{Operation, Value};
use soradyne::flow::flow_core::FlowConfig;
use soradyne::flow::types::drip_hosted::{DripHostPolicy, FullReplicaFlow};
use soradyne::identity::{CapsuleKeyBundle, DeviceIdentity};
use soradyne::topology::ensemble::TransportType;
use soradyne::topology::manager::{EnsembleConfig, EnsembleManager};

/// Default port for direct TCP connections.
const DEFAULT_PORT: u16 = 7117;

// ---------------------------------------------------------------------------
// Test-only key sharing (NOT part of rim protocol — see module doc)
// ---------------------------------------------------------------------------

/// Everything a second device needs to join this test capsule.
/// Serialized → base64 for copy-paste between terminals.
#[derive(Serialize, Deserialize)]
struct InviteCode {
    /// The shared capsule key bundle
    capsule_keys: CapsuleKeyBundle,
    /// The inviter's device UUID (so the joiner knows who the peer is)
    inviter_device_id: Uuid,
    /// The inviter's X25519 public key (for Noise IKpsk2)
    inviter_dh_public: [u8; 32],
    /// A shared flow UUID (both devices use the same one)
    flow_id: Uuid,
}

/// On-disk state persisted between runs.
#[derive(Serialize, Deserialize)]
struct PeerState {
    capsule_keys: CapsuleKeyBundle,
    flow_id: Uuid,
    /// Map of peer device_id → their DH public key
    peers: HashMap<Uuid, [u8; 32]>,
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".rim").join("mdns_sync_test")
}

fn identity_path() -> PathBuf {
    data_dir().join("device_identity.json")
}

fn state_path() -> PathBuf {
    data_dir().join("peer_state.json")
}

fn flow_storage_path() -> PathBuf {
    data_dir().join("flow_data")
}

fn addresses_path() -> PathBuf {
    data_dir().join("peers_addresses.json")
}

/// Load peer addresses from config file.
/// Format: `{"<uuid>": "ip:port", ...}`
fn load_peer_addresses() -> HashMap<Uuid, SocketAddr> {
    let path = addresses_path();
    if !path.exists() {
        return HashMap::new();
    }
    let json = match std::fs::read_to_string(&path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Warning: could not read {}: {}", path.display(), e);
            return HashMap::new();
        }
    };
    let raw: HashMap<String, String> = match serde_json::from_str(&json) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Warning: invalid JSON in {}: {}", path.display(), e);
            return HashMap::new();
        }
    };
    let mut out = HashMap::new();
    for (uuid_str, addr_str) in raw {
        let uuid = match uuid_str.parse::<Uuid>() {
            Ok(u) => u,
            Err(_) => {
                eprintln!("Warning: invalid UUID in peers_addresses.json: {}", uuid_str);
                continue;
            }
        };
        let addr = match addr_str.parse::<SocketAddr>() {
            Ok(a) => a,
            Err(_) => {
                // Try parsing as IP without port, append default port
                match addr_str.parse::<IpAddr>() {
                    Ok(ip) => SocketAddr::new(ip, DEFAULT_PORT),
                    Err(_) => {
                        eprintln!("Warning: invalid address in peers_addresses.json: {}", addr_str);
                        continue;
                    }
                }
            }
        };
        out.insert(uuid, addr);
    }
    out
}

fn load_or_create_identity() -> DeviceIdentity {
    let path = identity_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    DeviceIdentity::load_or_generate(&path).expect("Failed to load/generate device identity")
}

fn save_state(state: &PeerState) {
    let path = state_path();
    let json = serde_json::to_string_pretty(state).unwrap();
    std::fs::write(path, json).expect("Failed to save peer state");
}

fn load_state() -> Option<PeerState> {
    let path = state_path();
    if path.exists() {
        let json = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&json).ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Subcommands
// ---------------------------------------------------------------------------

fn cmd_init(identity: &DeviceIdentity) {
    if load_state().is_some() {
        eprintln!("Already initialized. Delete {} to start over.", data_dir().display());
        eprintln!("Or just run without arguments to start syncing.");
        return;
    }

    let capsule_id = Uuid::new_v4();
    let capsule_keys = CapsuleKeyBundle::generate(capsule_id);
    let flow_id = Uuid::new_v4();

    // Save our state (no peers yet — they'll be added when someone joins)
    let state = PeerState {
        capsule_keys: capsule_keys.clone(),
        flow_id,
        peers: HashMap::new(),
    };
    save_state(&state);

    // Build invite code for the other device
    let invite = InviteCode {
        capsule_keys,
        inviter_device_id: identity.device_id(),
        inviter_dh_public: identity.dh_public_bytes(),
        flow_id,
    };
    let json = serde_json::to_vec(&invite).unwrap();
    let b64 = to_hex(&json);

    println!();
    println!("=== INVITE CODE (copy this entire line to the other machine) ===");
    println!("{}", b64);
    println!("================================================================");
    println!();
    println!("This device: {}", identity.device_id());
    println!("Capsule:     {}", invite.capsule_keys.capsule_id);
    println!("Flow:        {}", invite.flow_id);
    println!();
    println!("After the other machine runs `join`, run `accept` with their response code:");
    println!("  cargo run --example mdns_sync_peer --no-default-features -- accept <RESPONSE_CODE>");
    println!();
    println!("For cross-network sync (e.g. Tailscale), create:");
    println!("  {}", addresses_path().display());
    println!("with content like:");
    println!("  {{\"<peer-uuid>\": \"<ip>:7117\", ...}}");
}

fn cmd_join(identity: &DeviceIdentity, invite_b64: &str) {
    if load_state().is_some() {
        eprintln!("Already initialized. Delete {} to start over.", data_dir().display());
        return;
    }

    let json = from_hex(invite_b64)
        .expect("Invalid hex in invite code");
    let invite: InviteCode =
        serde_json::from_slice(&json).expect("Invalid JSON in invite code");

    // Save state with the inviter as our only peer
    let mut peers = HashMap::new();
    peers.insert(invite.inviter_device_id, invite.inviter_dh_public);

    let state = PeerState {
        capsule_keys: invite.capsule_keys,
        flow_id: invite.flow_id,
        peers,
    };
    save_state(&state);

    // Print a response code so the inviter can add us
    #[derive(Serialize)]
    struct ResponseCode {
        joiner_device_id: Uuid,
        joiner_dh_public: [u8; 32],
    }
    let response = ResponseCode {
        joiner_device_id: identity.device_id(),
        joiner_dh_public: identity.dh_public_bytes(),
    };
    let resp_json = serde_json::to_vec(&response).unwrap();
    let resp_b64 = to_hex(&resp_json);

    println!();
    println!("Joined capsule {}.", state.capsule_keys.capsule_id);
    println!("Flow: {}", state.flow_id);
    println!();
    println!("=== RESPONSE CODE (copy this back to the inviting machine) ===");
    println!("{}", resp_b64);
    println!("===============================================================");
    println!();
    println!("On the inviting machine, run:");
    println!("  cargo run --example mdns_sync_peer --no-default-features -- accept {}", resp_b64);
}

fn cmd_accept(identity: &DeviceIdentity, response_b64: &str) {
    let mut state = load_state().expect("No state found. Run `init` first.");

    #[derive(Deserialize)]
    struct ResponseCode {
        joiner_device_id: Uuid,
        joiner_dh_public: [u8; 32],
    }

    let json = from_hex(response_b64)
        .expect("Invalid hex in response code");
    let response: ResponseCode =
        serde_json::from_slice(&json).expect("Invalid JSON in response code");

    state
        .peers
        .insert(response.joiner_device_id, response.joiner_dh_public);
    save_state(&state);

    println!();
    println!("Accepted peer {}.", response.joiner_device_id);
    println!("This device: {}", identity.device_id());
    println!("Ready to sync. Run without arguments to start.");
}

// ---------------------------------------------------------------------------
// Direct TCP connection helpers
// ---------------------------------------------------------------------------

/// After a raw TCP connection is established, both sides exchange their 16-byte
/// device UUIDs. This identifies the peer without mDNS, so we can register the
/// connection with the correct peer_id.
async fn do_uuid_handshake(
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    our_id: Uuid,
) -> Result<(LanConnection, Uuid), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Send our UUID
    stream
        .write_all(our_id.as_bytes())
        .await
        .map_err(|e| format!("write UUID: {e}"))?;

    // Read their UUID
    let mut peer_bytes = [0u8; 16];
    stream
        .read_exact(&mut peer_bytes)
        .await
        .map_err(|e| format!("read UUID: {e}"))?;
    let peer_id = Uuid::from_bytes(peer_bytes);

    let conn = LanConnection::from_stream(stream, BleAddress::Tcp(peer_addr));
    Ok((conn, peer_id))
}

/// Handle an incoming direct TCP connection: do UUID handshake, verify it's
/// a known peer, and register with the ensemble manager.
async fn handle_direct_tcp(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    manager: &Arc<EnsembleManager>,
    known_peers: &std::collections::HashSet<Uuid>,
    our_id: Uuid,
) -> Result<(), String> {
    let (conn, peer_id) = do_uuid_handshake(stream, peer_addr, our_id).await?;

    if !known_peers.contains(&peer_id) {
        return Err(format!("unknown peer {}", peer_id));
    }

    let conn: Arc<dyn BleConnection> = Arc::new(conn);
    manager
        .add_direct_connection(peer_id, conn, TransportType::TcpDirect)
        .await;
    println!("  Accepted direct TCP from {} ({})", &peer_id.to_string()[..8], peer_addr);
    Ok(())
}

/// Initiate a TCP connection to a peer: connect, handshake, register.
async fn connect_to_peer(
    manager: &Arc<EnsembleManager>,
    device_id: Uuid,
    peer_id: Uuid,
    peer_addr: SocketAddr,
) -> Result<(), String> {
    let stream = tokio::net::TcpStream::connect(peer_addr)
        .await
        .map_err(|e| format!("TCP connect to {}: {}", peer_addr, e))?;

    let (conn, confirmed_peer_id) = do_uuid_handshake(stream, peer_addr, device_id).await?;

    if confirmed_peer_id != peer_id {
        eprintln!(
            "  Warning: expected peer {} but got {} at {}",
            peer_id, confirmed_peer_id, peer_addr
        );
    }

    let conn: Arc<dyn BleConnection> = Arc::new(conn);
    manager
        .add_direct_connection(confirmed_peer_id, conn, TransportType::TcpDirect)
        .await;
    println!(
        "  Connected to {} via direct TCP",
        &confirmed_peer_id.to_string()[..8]
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Main sync loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let identity = load_or_create_identity();
    let args: Vec<String> = std::env::args().collect();

    // Parse --port flag from anywhere in args
    let mut listen_port: u16 = DEFAULT_PORT;
    let mut positional_args: Vec<String> = Vec::new();
    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--port" {
            listen_port = args
                .get(i + 1)
                .expect("--port requires a value")
                .parse()
                .expect("--port must be a number");
            skip_next = true;
        } else if arg.starts_with("--port=") {
            listen_port = arg[7..].parse().expect("--port must be a number");
        } else {
            positional_args.push(arg.clone());
        }
    }

    match positional_args.first().map(|s| s.as_str()) {
        Some("init") => {
            cmd_init(&identity);
            return;
        }
        Some("join") => {
            let code = positional_args.get(1).expect("Usage: ... join <INVITE_CODE>");
            cmd_join(&identity, code);
            return;
        }
        Some("accept") => {
            let code = positional_args.get(1).expect("Usage: ... accept <RESPONSE_CODE>");
            cmd_accept(&identity, code);
            return;
        }
        Some(other) => {
            eprintln!("Unknown command: {}", other);
            eprintln!("Usage: mdns_sync_peer [init | join <CODE> | accept <CODE>] [--port PORT]");
            return;
        }
        None => {} // Fall through to sync mode
    }

    // --- Sync mode ---
    let state = match load_state() {
        Some(s) => s,
        None => {
            eprintln!("No capsule configured. Run with `init` or `join` first.");
            return;
        }
    };

    if state.peers.is_empty() {
        eprintln!("No peers configured. Have the other device run `join` and then");
        eprintln!("run `accept <RESPONSE_CODE>` on this machine.");
        return;
    }

    let identity = Arc::new(identity);
    let device_id = identity.device_id();

    let peer_addresses = load_peer_addresses();

    println!("rim mDNS sync peer");
    println!("  Device:  {}", device_id);
    println!("  Capsule: {}", state.capsule_keys.capsule_id);
    println!("  Flow:    {}", state.flow_id);
    println!("  Listen:  0.0.0.0:{}", listen_port);
    println!("  Peers:   {}", state.peers.len());
    for (peer_id, _) in &state.peers {
        let addr_info = peer_addresses
            .get(peer_id)
            .map(|a| format!(" @ {}", a))
            .unwrap_or_default();
        println!("    - {}{}", peer_id, addr_info);
    }
    if peer_addresses.is_empty() {
        println!("  Direct addresses: (none — mDNS only)");
    }
    println!();

    // Build the ensemble manager
    let mut all_piece_ids: Vec<Uuid> = vec![device_id];
    all_piece_ids.extend(state.peers.keys());

    let manager = EnsembleManager::new(
        Arc::clone(&identity),
        state.capsule_keys.clone(),
        all_piece_ids,
        state.peers.clone(),
        EnsembleConfig::default(),
    );

    // Create the flow with persistent storage
    let flow_storage = flow_storage_path();
    std::fs::create_dir_all(&flow_storage).ok();

    let flow_config = FlowConfig {
        id: state.flow_id,
        type_name: "drip_hosted:inventory".to_string(),
        params: serde_json::json!({}),
    };

    let mut flow = FullReplicaFlow::new_persistent(
        flow_config,
        soradyne::convergent::inventory::InventorySchema,
        DripHostPolicy::default(),
        device_id,
        identity.device_id_string(),
        flow_storage,
    );

    // Register all peers as known parties
    for (peer_uuid, _) in &state.peers {
        flow.register_party(&peer_uuid.to_string(), *peer_uuid);
    }

    // Wire flow to ensemble
    use soradyne::flow::Flow;
    flow.set_ensemble(
        Arc::clone(manager.messenger()),
        Arc::clone(manager.topology()),
    );
    flow.start().expect("Failed to start flow sync");

    // Start mDNS transport
    let mdns = MdnsTransport::new(false).expect("Failed to create mDNS transport");

    // Create two devices — one for central role, one for peripheral role.
    // They share the same mDNS daemon so discovery works across both.
    let central_device = mdns.create_device();
    let peripheral_device = mdns.create_device();
    let central: Arc<dyn BleCentral> = Arc::new(central_device);
    let peripheral: Arc<dyn BlePeripheral> = Arc::new(peripheral_device);

    println!("Starting mDNS discovery...");
    manager.start(central, peripheral).await;

    // -----------------------------------------------------------------------
    // Direct TCP connections (for cross-network sync, e.g. Tailscale)
    // -----------------------------------------------------------------------

    // Accept incoming direct TCP connections on the listen port.
    // After TCP connect, both sides exchange 16-byte UUIDs to identify each other.
    let tcp_listener = tokio::net::TcpListener::bind(
        SocketAddr::new("0.0.0.0".parse().unwrap(), listen_port),
    )
    .await
    .expect("Failed to bind TCP listener");
    let actual_port = tcp_listener.local_addr().unwrap().port();
    println!("Direct TCP listening on port {}", actual_port);

    // Accept loop: incoming direct TCP connections
    {
        let manager = Arc::clone(&manager);
        let known_peers: std::collections::HashSet<Uuid> =
            state.peers.keys().cloned().collect();
        tokio::spawn(async move {
            loop {
                let (stream, peer_addr) = match tcp_listener.accept().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let manager = Arc::clone(&manager);
                let known_peers = known_peers.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_direct_tcp(stream, peer_addr, &manager, &known_peers, device_id)
                            .await
                    {
                        eprintln!("Direct TCP accept from {} failed: {}", peer_addr, e);
                    }
                });
            }
        });
    }

    // -----------------------------------------------------------------------
    // Outbound connections — everyone connects to everyone on startup.
    // No UUID tiebreaker: the last peer to come up always connects to
    // whoever is already running. Duplicate connections are harmless —
    // the messenger replaces the old one, and the old recv loop cleans
    // up safely (ptr_eq check prevents removing the replacement).
    // -----------------------------------------------------------------------
    for (&peer_id, &peer_addr) in &peer_addresses {
        let manager = Arc::clone(&manager);
        tokio::spawn(async move {
            println!("  Connecting to {} at {} ...", &peer_id.to_string()[..8], peer_addr);
            match connect_to_peer(&manager, device_id, peer_id, peer_addr).await {
                Ok(()) => {}
                Err(e) => {
                    // Not up yet — that's fine, they'll connect to us when they start.
                    eprintln!("  {} not reachable ({}), they will connect to us", &peer_id.to_string()[..8], e);
                }
            }
        });
    }

    // -----------------------------------------------------------------------
    // Reconnect on disconnect — event-driven, no polling.
    // When the messenger detects a peer dropped (recv error), it sends
    // the peer's UUID on the disconnect channel. We immediately try to
    // reconnect. If the peer is truly down, the connect fails and we
    // stop — they'll connect to US when they come back up (their startup
    // outbound connections handle it).
    // -----------------------------------------------------------------------
    {
        let mut disconnect_rx = manager.messenger().disconnections();
        let manager = Arc::clone(&manager);
        let peer_addresses = peer_addresses.clone();
        tokio::spawn(async move {
            loop {
                let peer_id = match disconnect_rx.recv().await {
                    Ok(id) => id,
                    Err(_) => break,
                };
                let Some(&peer_addr) = peer_addresses.get(&peer_id) else {
                    continue;
                };
                println!("  Peer {} disconnected — attempting reconnect", &peer_id.to_string()[..8]);
                let manager = Arc::clone(&manager);
                tokio::spawn(async move {
                    // Small delay to let the other side's socket close cleanly
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    match connect_to_peer(&manager, device_id, peer_id, peer_addr).await {
                        Ok(()) => {}
                        Err(e) => {
                            eprintln!("  Reconnect to {} failed ({}) — they will reconnect to us on restart",
                                &peer_id.to_string()[..8], e);
                        }
                    }
                });
            }
        });
    }

    println!();
    println!("Commands:");
    println!("  add <description>   — add an inventory item");
    println!("  list                — show all items");
    println!("  peers               — show connected peers");
    println!("  quit                — exit");
    println!();

    // REPL on stdin
    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        if line == "quit" || line == "exit" {
            break;
        } else if line == "list" {
            let doc = flow.document().read().unwrap();
            let state = doc.materialize();
            let items: Vec<_> = state.iter_existing().collect();
            if items.is_empty() {
                println!("  (no items)");
            } else {
                for (id, item) in items {
                    let desc = item
                        .fields
                        .get("description")
                        .and_then(|v| match v {
                            Value::String(s) => Some(s.as_str()),
                            _ => None,
                        })
                        .unwrap_or("(no description)");
                    println!("  [{}] {}", &id[..8.min(id.len())], desc);
                }
            }
        } else if line == "peers" {
            match manager.topology().try_read() {
                Ok(topo) => {
                    let pieces: Vec<_> = topo.online_pieces.keys().collect();
                    if pieces.is_empty() {
                        println!("  (no peers connected)");
                    } else {
                        for id in pieces {
                            let label = if *id == device_id { " (self)" } else { "" };
                            println!("  {}{}", id, label);
                        }
                    }
                }
                Err(_) => println!("  (topology busy, try again)"),
            }
        } else if let Some(desc) = line.strip_prefix("add ") {
            let desc = desc.trim();
            if desc.is_empty() {
                println!("  Usage: add <description>");
                continue;
            }
            let item_id = format!("item_{}", &Uuid::new_v4().to_string()[..8]);
            flow.apply_edit(Operation::add_item(&item_id, "InventoryItem"))
                .expect("apply_edit failed");
            flow.apply_edit(Operation::set_field(
                &item_id,
                "description",
                Value::string(desc),
            ))
            .expect("set_field failed");
            println!("  Added: [{}] {}", &item_id[..12], desc);
        } else if line == "clear" {
            let doc = flow.document().read().unwrap();
            let ids: Vec<String> = doc.materialize().iter_existing()
                .map(|(id, _)| id.to_string())
                .collect();
            drop(doc);
            for id in &ids {
                flow.apply_edit(Operation::remove_item(id))
                    .expect("remove failed");
            }
            println!("  Removed {} items", ids.len());
        } else {
            println!("  Unknown command. Try: add <desc>, list, peers, clear, quit");
        }

        print!("> ");
        io::stdout().flush().ok();
    }

    println!("Shutting down...");
    manager.stop();
    flow.stop();
}
