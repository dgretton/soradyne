use std::path::PathBuf;

use clap::{Parser, Subcommand};
use uuid::Uuid;

use soradyne::ffi::convergent_flow_ffi::ConvergentFlow;
use soradyne::identity::{CapsuleKeyBundle, DeviceIdentity};
use soradyne::topology::{
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
}

/// Data exchanged when inviting a peer.
#[derive(serde::Serialize, serde::Deserialize)]
struct InviteCode {
    capsule_keys: CapsuleKeyBundle,
    inviter_device_id: Uuid,
    inviter_verifying_key: [u8; 32],
    inviter_dh_public: [u8; 32],
}

/// Data sent back by the joining peer.
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
    let cli = Cli::parse();
    let base = data_dir(&cli);

    match cli.command {
        Commands::Capsule { action } => handle_capsule(action, &base),
        Commands::Flow { action } => handle_flow(action, &base),
        Commands::DeviceId => {
            let identity = load_identity(&base);
            println!("{}", identity.device_id());
        }
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
    }
}
