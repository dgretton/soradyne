use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use uuid::Uuid;

use soradyne::ble::simulated::SimBleNetwork;
use soradyne::convergent::{Operation, Value};
use soradyne::ffi::convergent_flow_ffi::ConvergentFlow;
use soradyne::identity::{CapsuleKeyBundle, DeviceIdentity};
use soradyne::topology::{
    manager::{EnsembleConfig, EnsembleManager},
    Capsule, CapsuleStore, PieceCapabilities, PieceRecord, PieceRole, StaticPeerConfig,
};

/// Soradyne CLI — capsule management and flow inspection.
#[derive(Parser)]
#[command(name = "soradyne-cli")]
struct Cli {
    /// Data directory (default: ~/.soradyne)
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage capsules (trust groups of devices)
    Capsule {
        #[command(subcommand)]
        action: CapsuleAction,
    },
    /// Inspect flow state
    Flow {
        #[command(subcommand)]
        action: FlowAction,
    },
    /// Print this device's UUID
    DeviceId,
    /// Start sync for all local flows (long-running)
    Sync,
}

#[derive(Subcommand)]
enum CapsuleAction {
    /// Create a new capsule and print its invite code
    Create {
        /// Human-friendly name for the capsule
        #[arg(long, default_value = "default")]
        name: String,
    },
    /// List all capsules
    List,
    /// Export an invite code for a capsule (for another device to join)
    Export {
        /// Capsule UUID
        capsule_id: String,
    },
    /// Import an invite code from another device
    Import {
        /// Hex-encoded invite code
        code: String,
    },
    /// Accept a response code from a device that imported your invite
    AcceptResponse {
        /// Capsule UUID
        capsule_id: String,
        /// Hex-encoded response code from the joining device
        code: String,
    },
    /// Add a static peer address (e.g. Tailscale IP) for a capsule
    AddPeer {
        /// Capsule UUID
        capsule_id: String,
        /// Peer device UUID
        peer_id: String,
        /// IP:port address (e.g. 100.64.0.2:7979)
        address: String,
    },
    /// Remove a static peer address
    RemovePeer {
        /// Capsule UUID
        capsule_id: String,
        /// Peer device UUID
        peer_id: String,
    },
    /// List static peer addresses for a capsule
    Peers {
        /// Capsule UUID
        capsule_id: String,
    },
}

#[derive(Subcommand)]
enum FlowAction {
    /// Read and print the current state of a flow
    Inspect {
        /// Flow UUID
        uuid: String,
        /// Schema: "giantt" or "inventory"
        #[arg(long, default_value = "giantt")]
        schema: String,
    },
    /// Create a new empty flow and associate it with a capsule
    Create {
        /// Capsule UUID to associate with
        #[arg(long)]
        capsule: String,
        /// Schema: "giantt" or "inventory"
        #[arg(long, default_value = "giantt")]
        schema: String,
    },
    /// Add an item to a flow
    AddItem {
        /// Flow UUID
        uuid: String,
        /// Item ID (unique within the flow)
        item_id: String,
        /// Item title
        title: String,
        /// Schema: "giantt" or "inventory"
        #[arg(long, default_value = "giantt")]
        schema: String,
    },
    /// List all flows in the data directory
    List,
}

/// Data exchanged when inviting a peer.
///
/// NOTE: This is a simplified development pairing model. Key material
/// (CapsuleKeyBundle, DH public keys) is passed directly as hex-encoded
/// JSON, with no confidentiality or authentication of the exchange itself.
///
/// The real pairing flow (implemented in PairingEngine for BLE) uses:
/// 1. X25519 ECDH key agreement over an unauthenticated channel
/// 2. 6-digit PIN derived from the shared secret (SHA-256) for
///    out-of-band confirmation by both users
/// 3. AES-256-GCM encryption of the CapsuleKeyBundle under the ECDH
///    shared secret, sent only after PIN confirmation
///
/// To bring this CLI flow up to the real protocol, the export/import
/// exchange would need to be replaced with an interactive session
/// (e.g., over TCP or a relay) that performs the ECDH handshake and
/// PIN confirmation before transferring key material. The current
/// approach is equivalent to trusting the transport (terminal
/// copy-paste) to be confidential, which is acceptable for local
/// development but not for production use.
#[derive(serde::Serialize, serde::Deserialize)]
struct InviteCode {
    capsule_keys: CapsuleKeyBundle,
    inviter_device_id: Uuid,
    inviter_verifying_key: [u8; 32],
    inviter_dh_public: [u8; 32],
}

/// Data sent back by the joining peer.
///
/// See [InviteCode] for notes on the simplified pairing model.
#[derive(serde::Serialize, serde::Deserialize)]
struct ResponseCode {
    joiner_device_id: Uuid,
    joiner_verifying_key: [u8; 32],
    joiner_dh_public: [u8; 32],
}

fn data_dir(cli: &Cli) -> PathBuf {
    cli.data_dir.clone().unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".soradyne")
    })
}

fn load_identity(base: &PathBuf) -> DeviceIdentity {
    let id_path = base.join("device_identity.json");
    std::fs::create_dir_all(base).expect("failed to create data directory");
    DeviceIdentity::load_or_generate(&id_path).expect("failed to load or generate device identity")
}

fn load_capsule_store(base: &PathBuf) -> CapsuleStore {
    let capsules_dir = base.join("capsules");
    std::fs::create_dir_all(&capsules_dir).expect("failed to create capsules directory");
    CapsuleStore::load(&capsules_dir).unwrap_or_else(|_| CapsuleStore::new(capsules_dir))
}

fn hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".into())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let cli = Cli::parse();
    let base = data_dir(&cli);

    match cli.command {
        Commands::Capsule { action } => handle_capsule(action, &base),
        Commands::Flow { action } => handle_flow(action, &base),
        Commands::DeviceId => {
            let identity = load_identity(&base);
            println!("{}", identity.device_id());
        }
        Commands::Sync => handle_sync(&base),
    }
}

fn handle_capsule(action: CapsuleAction, base: &PathBuf) {
    let identity = load_identity(base);

    match action {
        CapsuleAction::Create { name } => {
            let mut store = load_capsule_store(base);
            let keys = CapsuleKeyBundle::generate(Uuid::new_v4());
            let capsule_id = keys.capsule_id;

            let mut capsule = Capsule::new(name.clone(), keys.clone());
            capsule.pieces.push(PieceRecord::from_identity(
                &identity,
                hostname(),
                PieceCapabilities::full(),
                PieceRole::Full,
            ));

            store
                .insert_capsule(capsule)
                .expect("failed to save capsule");

            let invite = InviteCode {
                capsule_keys: keys,
                inviter_device_id: identity.device_id(),
                inviter_verifying_key: identity.verifying_key_bytes(),
                inviter_dh_public: identity.dh_public_bytes(),
            };
            let invite_hex = hex::encode(serde_json::to_vec(&invite).unwrap());

            println!("Created capsule \"{}\" ({})", name, capsule_id);
            println!();
            println!("Share this invite code with other devices:");
            println!("{}", invite_hex);
        }

        CapsuleAction::List => {
            let store = load_capsule_store(base);
            let capsules = store.list_capsules();
            if capsules.is_empty() {
                println!("(no capsules)");
                return;
            }
            for c in capsules {
                println!(
                    "  {} \"{}\"  ({} pieces, {} flows, {:?})",
                    c.id,
                    c.name,
                    c.pieces.len(),
                    c.flows.len(),
                    c.status,
                );
            }
        }

        CapsuleAction::Export { capsule_id } => {
            let store = load_capsule_store(base);
            let id = Uuid::parse_str(&capsule_id).expect("invalid UUID");
            let capsule = store
                .get_capsule(&id)
                .unwrap_or_else(|| panic!("capsule {} not found", id));

            let invite = InviteCode {
                capsule_keys: capsule.keys.clone(),
                inviter_device_id: identity.device_id(),
                inviter_verifying_key: identity.verifying_key_bytes(),
                inviter_dh_public: identity.dh_public_bytes(),
            };
            let invite_hex = hex::encode(serde_json::to_vec(&invite).unwrap());

            println!(
                "Invite code for capsule \"{}\" ({}):",
                capsule.name, capsule.id
            );
            println!("{}", invite_hex);
        }

        CapsuleAction::Import { code } => {
            let bytes = hex::decode(&code).expect("invalid hex");
            let invite: InviteCode =
                serde_json::from_slice(&bytes).expect("invalid invite code JSON");

            let mut store = load_capsule_store(base);
            let capsule_id = invite.capsule_keys.capsule_id;

            let mut capsule = Capsule::new("imported".into(), invite.capsule_keys);

            // Add the inviter as a piece
            capsule.pieces.push(PieceRecord {
                device_id: invite.inviter_device_id,
                name: "inviter".into(),
                verifying_key: invite.inviter_verifying_key,
                dh_public_key: invite.inviter_dh_public,
                added_at: chrono::Utc::now(),
                capabilities: PieceCapabilities::full(),
                role: PieceRole::Full,
            });

            // Add ourselves
            capsule.pieces.push(PieceRecord::from_identity(
                &identity,
                hostname(),
                PieceCapabilities::full(),
                PieceRole::Full,
            ));

            store
                .insert_capsule(capsule)
                .expect("failed to save capsule");

            // Print response for the inviter
            let response = ResponseCode {
                joiner_device_id: identity.device_id(),
                joiner_verifying_key: identity.verifying_key_bytes(),
                joiner_dh_public: identity.dh_public_bytes(),
            };
            let response_hex = hex::encode(serde_json::to_vec(&response).unwrap());

            println!("Imported capsule {}", capsule_id);
            println!();
            println!("Send this response back to the inviter:");
            println!("{}", response_hex);
        }

        CapsuleAction::AcceptResponse { capsule_id, code } => {
            let id = Uuid::parse_str(&capsule_id).expect("invalid capsule UUID");
            let bytes = hex::decode(&code).expect("invalid hex");
            let response: ResponseCode =
                serde_json::from_slice(&bytes).expect("invalid response code JSON");

            let mut store = load_capsule_store(base);

            let piece = PieceRecord {
                device_id: response.joiner_device_id,
                name: "joiner".into(),
                verifying_key: response.joiner_verifying_key,
                dh_public_key: response.joiner_dh_public,
                added_at: chrono::Utc::now(),
                capabilities: PieceCapabilities::full(),
                role: PieceRole::Full,
            };

            match store.add_piece(&id, piece) {
                Ok(true) => {
                    println!(
                        "Added device {} to capsule {}",
                        response.joiner_device_id, capsule_id
                    );
                }
                Ok(false) => {
                    println!("Device {} already in capsule", response.joiner_device_id);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        CapsuleAction::AddPeer {
            capsule_id,
            peer_id,
            address,
        } => {
            let cid = Uuid::parse_str(&capsule_id).expect("invalid capsule UUID");
            let pid = Uuid::parse_str(&peer_id).expect("invalid peer UUID");
            let addr: std::net::SocketAddr = address.parse().expect("invalid address (expected ip:port)");

            let mut config = StaticPeerConfig::load(base).expect("failed to load static peer config");
            config
                .set_peer(cid, pid, addr)
                .expect("failed to save static peer config");

            println!("Added static peer {} → {} for capsule {}", pid, addr, cid);
        }

        CapsuleAction::RemovePeer {
            capsule_id,
            peer_id,
        } => {
            let cid = Uuid::parse_str(&capsule_id).expect("invalid capsule UUID");
            let pid = Uuid::parse_str(&peer_id).expect("invalid peer UUID");

            let mut config = StaticPeerConfig::load(base).expect("failed to load static peer config");
            config
                .remove_peer(&cid, &pid)
                .expect("failed to save static peer config");

            println!("Removed static peer {} from capsule {}", pid, cid);
        }

        CapsuleAction::Peers { capsule_id } => {
            let cid = Uuid::parse_str(&capsule_id).expect("invalid capsule UUID");
            let config = StaticPeerConfig::load(base).expect("failed to load static peer config");
            let peers = config.get(&cid);

            if peers.is_empty() {
                println!("(no static peers for capsule {})", cid);
            } else {
                println!("Static peers for capsule {}:", cid);
                for (peer_id, addr) in &peers {
                    println!("  {} → {}", peer_id, addr);
                }
            }
        }
    }
}

fn handle_flow(action: FlowAction, base: &PathBuf) {
    let identity = load_identity(base);

    match action {
        FlowAction::Inspect { uuid, schema } => {
            let flow_dir = base.join("flows").join(&uuid);
            let flow_uuid = Uuid::parse_str(&uuid).unwrap_or_else(|_| {
                eprintln!("warning: invalid UUID \"{}\", using random", uuid);
                Uuid::new_v4()
            });

            let device_id: soradyne::convergent::DeviceId =
                identity.device_id().to_string().into();

            match ConvergentFlow::new_persistent(&schema, device_id, flow_dir, flow_uuid) {
                Some(flow) => {
                    let state = flow.read_drip();
                    if state.is_empty() {
                        println!("(empty flow)");
                    } else {
                        println!("{}", state);
                    }
                }
                None => {
                    eprintln!(
                        "Unknown schema: \"{}\". Use \"giantt\" or \"inventory\".",
                        schema
                    );
                    std::process::exit(1);
                }
            }
        }

        FlowAction::Create { capsule, schema } => {
            let capsule_id = Uuid::parse_str(&capsule).expect("invalid capsule UUID");

            // Verify the capsule exists
            let store = load_capsule_store(base);
            if store.get_capsule(&capsule_id).is_none() {
                eprintln!("Capsule {} not found.", capsule_id);
                std::process::exit(1);
            }

            let flow_uuid = Uuid::new_v4();
            let flow_dir = base.join("flows").join(flow_uuid.to_string());
            std::fs::create_dir_all(&flow_dir).expect("failed to create flow directory");

            // Write capsule_id association
            std::fs::write(flow_dir.join("capsule_id"), capsule_id.to_string())
                .expect("failed to write capsule_id");

            // Initialize the flow so its journal directory exists
            let device_id: soradyne::convergent::DeviceId =
                identity.device_id().to_string().into();
            if ConvergentFlow::new_persistent(&schema, device_id, flow_dir, flow_uuid).is_none() {
                eprintln!(
                    "Unknown schema: \"{}\". Use \"giantt\" or \"inventory\".",
                    schema
                );
                std::process::exit(1);
            }

            println!("{}", flow_uuid);
        }

        FlowAction::AddItem {
            uuid,
            item_id,
            title,
            schema,
        } => {
            let flow_uuid = Uuid::parse_str(&uuid).expect("invalid flow UUID");
            let flow_dir = base.join("flows").join(&uuid);
            if !flow_dir.exists() {
                eprintln!("Flow directory not found: {}", flow_dir.display());
                std::process::exit(1);
            }

            let device_id: soradyne::convergent::DeviceId =
                identity.device_id().to_string().into();

            let item_type = match schema.as_str() {
                "giantt" => "GianttItem",
                "inventory" => "InventoryItem",
                _ => {
                    eprintln!(
                        "Unknown schema: \"{}\". Use \"giantt\" or \"inventory\".",
                        schema
                    );
                    std::process::exit(1);
                }
            };

            let mut flow = ConvergentFlow::new_persistent(&schema, device_id, flow_dir, flow_uuid)
                .expect("failed to open flow");

            flow.apply_operation(Operation::AddItem {
                item_id: item_id.clone(),
                item_type: item_type.to_string(),
            });
            flow.apply_operation(Operation::SetField {
                item_id: item_id.clone(),
                field: "title".to_string(),
                value: Value::string(&title),
            });

            println!("Added item \"{}\" ({}) to flow {}", item_id, title, flow_uuid);
        }

        FlowAction::List => {
            let flows_dir = base.join("flows");
            if !flows_dir.is_dir() {
                println!("(no flows)");
                return;
            }

            let mut entries: Vec<_> = std::fs::read_dir(&flows_dir)
                .expect("failed to read flows directory")
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map_or(false, |ft| ft.is_dir()))
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    Uuid::parse_str(&name).ok().map(|uuid| (uuid, e.path()))
                })
                .collect();

            if entries.is_empty() {
                println!("(no flows)");
                return;
            }

            entries.sort_by_key(|(uuid, _)| *uuid);

            for (uuid, path) in &entries {
                let capsule_id = std::fs::read_to_string(path.join("capsule_id"))
                    .ok()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "(none)".to_string());
                let schema = detect_flow_schema(path);
                println!("  {} schema={} capsule={}", uuid, schema, capsule_id);
            }
        }
    }
}

/// Detect the schema for a flow by reading the first AddItem in its journal.
///
/// Falls back to "giantt" if no journal exists or no AddItem is found.
fn detect_flow_schema(flow_dir: &std::path::Path) -> &'static str {
    let journals_dir = flow_dir.join("journals");
    if let Ok(entries) = std::fs::read_dir(&journals_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().map_or(false, |e| e == "jsonl") {
                if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                    if let Some(first_line) = contents.lines().next() {
                        if first_line.contains("\"InventoryItem\"") {
                            return "inventory";
                        }
                    }
                }
            }
        }
    }
    "giantt"
}

fn handle_sync(base: &PathBuf) {
    let identity = load_identity(base);
    let store = load_capsule_store(base);
    let capsules = store.list_capsules();

    if capsules.is_empty() {
        eprintln!("No capsules found. Create one with `soradyne-cli capsule create`.");
        std::process::exit(1);
    }

    // Discover flows
    let flows_dir = base.join("flows");
    let flow_dirs: Vec<_> = if flows_dir.is_dir() {
        std::fs::read_dir(&flows_dir)
            .expect("failed to read flows directory")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map_or(false, |ft| ft.is_dir()))
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                Uuid::parse_str(&name).ok().map(|uuid| (uuid, e.path()))
            })
            .collect()
    } else {
        Vec::new()
    };

    if flow_dirs.is_empty() {
        eprintln!("No flows found in {}.", flows_dir.display());
        eprintln!("Flows are created when apps write data (e.g. `giantt add ...`).");
        std::process::exit(1);
    }

    let device_id_str: soradyne::convergent::DeviceId =
        identity.device_id().to_string().into();
    let identity_arc = Arc::new(identity);

    // Build a tokio runtime for ensemble + flow sync tasks
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("soradyne-sync")
        .build()
        .expect("failed to build Tokio runtime");

    // For each capsule, create an EnsembleManager and start it
    let mut managers: HashMap<Uuid, Arc<EnsembleManager>> = HashMap::new();

    for capsule in &capsules {
        let piece_ids: Vec<Uuid> = capsule.pieces.iter().map(|p| p.device_id).collect();

        let peer_static_keys: HashMap<Uuid, [u8; 32]> = capsule
            .pieces
            .iter()
            .filter(|p| p.device_id != identity_arc.device_id())
            .map(|p| (p.device_id, p.dh_public_key))
            .collect();

        let static_peers = StaticPeerConfig::load(base)
            .map(|cfg| cfg.get(&capsule.id))
            .unwrap_or_default();

        let config = EnsembleConfig {
            static_peers,
            ..EnsembleConfig::default()
        };

        let manager = EnsembleManager::new(
            Arc::clone(&identity_arc),
            capsule.keys.clone(),
            piece_ids,
            peer_static_keys,
            config,
        );

        // Each manager gets its own isolated SimBleNetwork so managers
        // can't accidentally connect via SimBLE (which has a 247-byte MTU
        // too small for horizon exchange). All real sync goes through TCP.
        let sim = SimBleNetwork::new();
        let central = sim.create_device();
        let peripheral = sim.create_device();
        runtime.block_on(manager.start(Arc::new(central), Arc::new(peripheral)));

        println!(
            "Ensemble started for capsule \"{}\" ({}, {} pieces)",
            capsule.name,
            capsule.id,
            capsule.pieces.len(),
        );

        managers.insert(capsule.id, manager);
    }

    // Enter the runtime context so tokio::spawn works in flow.start()
    let _runtime_guard = runtime.enter();

    // Wire each flow to its capsule's ensemble manager.
    // The capsule_id file in each flow directory records the association.
    // If missing, fall back to the first capsule with 2+ pieces.
    let fallback_capsule_id = capsules
        .iter()
        .filter(|c| c.pieces.len() >= 2)
        .map(|c| c.id)
        .next()
        .unwrap_or(capsules[0].id);

    let mut flow_count = 0;

    for (flow_uuid, flow_path) in &flow_dirs {
        let schema = detect_flow_schema(flow_path);

        // Determine which capsule this flow belongs to
        let capsule_id = std::fs::read_to_string(flow_path.join("capsule_id"))
            .ok()
            .and_then(|s| Uuid::parse_str(s.trim()).ok())
            .unwrap_or(fallback_capsule_id);

        let manager = match managers.get(&capsule_id) {
            Some(m) => m,
            None => {
                eprintln!(
                    "Warning: flow {} references capsule {} which has no manager; skipping",
                    flow_uuid, capsule_id,
                );
                continue;
            }
        };

        let mut flow = match ConvergentFlow::new_persistent(
            schema,
            device_id_str.clone(),
            flow_path.clone(),
            *flow_uuid,
        ) {
            Some(f) => f,
            None => {
                eprintln!("Warning: failed to open flow {} (schema: {})", flow_uuid, schema);
                continue;
            }
        };

        flow.set_ensemble(
            Arc::clone(manager.messenger()),
            Arc::clone(manager.topology()),
        );

        match flow.start() {
            Ok(()) => {
                println!("Sync started for flow {} ({}) → capsule {}", flow_uuid, schema, capsule_id);
                flow_count += 1;
            }
            Err(e) => {
                eprintln!("Warning: failed to start sync for flow {}: {:?}", flow_uuid, e);
            }
        }

        // Keep the flow alive by leaking it into an Arc
        // (flows must remain in memory for background sync tasks to run)
        let _keep_alive = Arc::new(Mutex::new(flow));
        std::mem::forget(_keep_alive);
    }

    println!();
    println!(
        "Syncing {} flow(s) across {} capsule(s). Press Ctrl+C to stop.",
        flow_count,
        managers.len(),
    );

    // Block until Ctrl+C
    runtime.block_on(async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for Ctrl+C");
    });

    println!("\nShutting down...");
    for (_, manager) in &managers {
        manager.stop();
    }
}
