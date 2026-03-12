//! mdns_sync_peer — CLI tool for testing mDNS-discovered CRDT sync between machines.
//!
//! Usage:
//!   # On machine A (first time — generates a capsule and prints invite code):
//!   cargo run --example mdns_sync_peer --no-default-features -- init
//!
//!   # On machine B (paste the invite code from machine A):
//!   cargo run --example mdns_sync_peer --no-default-features -- join <INVITE_CODE>
//!
//!   # On either machine (after init/join — starts syncing):
//!   cargo run --example mdns_sync_peer --no-default-features
//!
//! The REPL supports: add <desc>, list, quit
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

use soradyne::ble::mdns_transport::MdnsTransport;
use soradyne::ble::transport::{BleCentral, BlePeripheral};
use soradyne::convergent::{Operation, Value};
use soradyne::flow::flow_core::FlowConfig;
use soradyne::flow::types::drip_hosted::{DripHostPolicy, FullReplicaFlow};
use soradyne::identity::{CapsuleKeyBundle, DeviceIdentity};
use soradyne::topology::manager::{EnsembleConfig, EnsembleManager};

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
    println!("After the other machine runs `join`, re-run this command with");
    println!("their response code to complete the handshake:");
    println!("  cargo run --example mdns_sync_peer --no-default-features -- accept <RESPONSE_CODE>");
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
// Main sync loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let identity = load_or_create_identity();
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("init") => {
            cmd_init(&identity);
            return;
        }
        Some("join") => {
            let code = args.get(2).expect("Usage: ... join <INVITE_CODE>");
            cmd_join(&identity, code);
            return;
        }
        Some("accept") => {
            let code = args.get(2).expect("Usage: ... accept <RESPONSE_CODE>");
            cmd_accept(&identity, code);
            return;
        }
        Some(other) => {
            eprintln!("Unknown command: {}", other);
            eprintln!("Usage: mdns_sync_peer [init | join <CODE> | accept <CODE>]");
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

    println!("rim mDNS sync peer");
    println!("  Device:  {}", device_id);
    println!("  Capsule: {}", state.capsule_keys.capsule_id);
    println!("  Flow:    {}", state.flow_id);
    println!("  Peers:   {}", state.peers.len());
    for (peer_id, _) in &state.peers {
        println!("    - {}", peer_id);
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
    println!("Discovery running. Type commands below.");
    println!();
    println!("Commands:");
    println!("  add <description>   — add an inventory item");
    println!("  list                — show all items");
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
        } else {
            println!("  Unknown command. Try: add <desc>, list, quit");
        }

        print!("> ");
        io::stdout().flush().ok();
    }

    println!("Shutting down...");
    manager.stop();
    flow.stop();
}
