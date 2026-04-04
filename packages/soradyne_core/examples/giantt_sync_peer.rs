//! giantt_sync_peer — CLI tool for syncing Giantt task graphs between machines.
//!
//! Shares the same capsule/key infrastructure as mdns_sync_peer (see that
//! example for setup docs). Uses `~/.rim/giantt_sync/` for state.
//!
//! Usage:
//!   cargo run --example giantt_sync_peer --no-default-features -- init
//!   cargo run --example giantt_sync_peer --no-default-features -- join <CODE>
//!   cargo run --example giantt_sync_peer --no-default-features -- accept <CODE>
//!   cargo run --example giantt_sync_peer --no-default-features -- --port 7118
//!
//! REPL commands:
//!   import <path>     — import items from a .giantt file
//!   list              — show all tasks
//!   add <title>       — add a new task
//!   status <id> <s>   — set status (todo/wip/done/blocked)
//!   get <id>          — show all fields for a task
//!   clear             — remove all items
//!   peers             — show connected peers
//!   quit              — exit
//!
//! ## On key sharing
//!
//! Same caveat as mdns_sync_peer: the init/join/accept commands are a test
//! shim for bootstrapping capsule keys without Bluetooth. NOT part of the
//! rim protocol.

use std::collections::HashMap;
use std::io::{self, BufRead, Write as _};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

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

const DEFAULT_PORT: u16 = 7118;

// ---------------------------------------------------------------------------
// Test-only key sharing (NOT part of rim protocol)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct InviteCode {
    capsule_keys: CapsuleKeyBundle,
    inviter_device_id: Uuid,
    inviter_dh_public: [u8; 32],
    flow_id: Uuid,
}

#[derive(Serialize, Deserialize)]
struct PeerState {
    capsule_keys: CapsuleKeyBundle,
    flow_id: Uuid,
    peers: HashMap<Uuid, [u8; 32]>,
}

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".rim").join("giantt_sync")
}

fn identity_path() -> PathBuf { data_dir().join("device_identity.json") }
fn state_path() -> PathBuf { data_dir().join("peer_state.json") }
fn flow_storage_path() -> PathBuf { data_dir().join("flow_data") }
fn addresses_path() -> PathBuf { data_dir().join("peers_addresses.json") }

fn load_or_create_identity() -> DeviceIdentity {
    let path = identity_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    DeviceIdentity::load_or_generate(&path).expect("Failed to load/generate device identity")
}

fn save_state(state: &PeerState) {
    let json = serde_json::to_string_pretty(state).unwrap();
    std::fs::write(state_path(), json).expect("Failed to save peer state");
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

fn load_peer_addresses() -> HashMap<Uuid, SocketAddr> {
    let path = addresses_path();
    if !path.exists() { return HashMap::new(); }
    let json = match std::fs::read_to_string(&path) {
        Ok(j) => j,
        Err(_) => return HashMap::new(),
    };
    let raw: HashMap<String, String> = match serde_json::from_str(&json) {
        Ok(m) => m,
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    for (uuid_str, addr_str) in raw {
        let uuid = match uuid_str.parse::<Uuid>() { Ok(u) => u, Err(_) => continue };
        let addr = match addr_str.parse::<SocketAddr>() {
            Ok(a) => a,
            Err(_) => match addr_str.parse::<IpAddr>() {
                Ok(ip) => SocketAddr::new(ip, DEFAULT_PORT),
                Err(_) => continue,
            },
        };
        out.insert(uuid, addr);
    }
    out
}

// ---------------------------------------------------------------------------
// Subcommands (identical to mdns_sync_peer)
// ---------------------------------------------------------------------------

fn cmd_init(identity: &DeviceIdentity) {
    if load_state().is_some() {
        eprintln!("Already initialized. Delete {} to start over.", data_dir().display());
        return;
    }
    let capsule_keys = CapsuleKeyBundle::generate(Uuid::new_v4());
    let flow_id = Uuid::new_v4();
    save_state(&PeerState { capsule_keys: capsule_keys.clone(), flow_id, peers: HashMap::new() });

    let invite = InviteCode {
        capsule_keys,
        inviter_device_id: identity.device_id(),
        inviter_dh_public: identity.dh_public_bytes(),
        flow_id,
    };
    let b64 = to_hex(&serde_json::to_vec(&invite).unwrap());
    println!("\n=== INVITE CODE ===\n{}\n===================", b64);
    println!("Device: {}\nAfter join on the other machine, run: accept <RESPONSE_CODE>", identity.device_id());
    println!("For cross-network sync, create: {}", addresses_path().display());
}

fn cmd_join(identity: &DeviceIdentity, invite_hex: &str) {
    if load_state().is_some() {
        eprintln!("Already initialized. Delete {} to start over.", data_dir().display());
        return;
    }
    let invite: InviteCode = serde_json::from_slice(
        &from_hex(invite_hex).expect("Invalid hex"),
    ).expect("Invalid invite JSON");

    let mut peers = HashMap::new();
    peers.insert(invite.inviter_device_id, invite.inviter_dh_public);
    save_state(&PeerState { capsule_keys: invite.capsule_keys, flow_id: invite.flow_id, peers });

    #[derive(Serialize)]
    struct ResponseCode { joiner_device_id: Uuid, joiner_dh_public: [u8; 32] }
    let resp = to_hex(&serde_json::to_vec(&ResponseCode {
        joiner_device_id: identity.device_id(),
        joiner_dh_public: identity.dh_public_bytes(),
    }).unwrap());
    println!("\n=== RESPONSE CODE ===\n{}\n=====================", resp);
}

fn cmd_accept(_identity: &DeviceIdentity, response_hex: &str) {
    let mut state = load_state().expect("No state found. Run `init` first.");
    #[derive(Deserialize)]
    struct ResponseCode { joiner_device_id: Uuid, joiner_dh_public: [u8; 32] }
    let resp: ResponseCode = serde_json::from_slice(
        &from_hex(response_hex).expect("Invalid hex"),
    ).expect("Invalid response JSON");
    state.peers.insert(resp.joiner_device_id, resp.joiner_dh_public);
    save_state(&state);
    println!("Accepted peer {}. Ready to sync.", resp.joiner_device_id);
}

// ---------------------------------------------------------------------------
// .giantt file parser
// ---------------------------------------------------------------------------

/// A parsed Giantt item from a .giantt text file.
struct ParsedGianttItem {
    id: String,
    title: String,
    status: String,
    priority: String,
    duration: Option<String>,
    charts: Vec<String>,
    tags: Vec<String>,
    relations: Vec<(String, Vec<String>)>, // (relation_type, target_ids)
    time_constraints: Vec<String>,
    comment: Option<String>,
}

/// Parse a single .giantt item line. Returns None for comments/blanks/headers.
fn parse_giantt_line(line: &str) -> Option<ParsedGianttItem> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Status symbol is the first character(s)
    let (status, rest) = if line.starts_with('○') || line.starts_with("○") {
        ("NOT_STARTED".to_string(), line.trim_start_matches('○').trim_start())
    } else if line.starts_with('◑') {
        ("IN_PROGRESS".to_string(), line.trim_start_matches('◑').trim_start())
    } else if line.starts_with('⊘') {
        ("BLOCKED".to_string(), line.trim_start_matches('⊘').trim_start())
    } else if line.starts_with('●') {
        ("COMPLETED".to_string(), line.trim_start_matches('●').trim_start())
    } else {
        return None; // Not a valid item line
    };

    // Split off comments first
    let (main_part, comment) = if let Some(idx) = rest.find(" # ") {
        let comment_text = rest[idx + 3..].trim();
        // Handle ### auto-comments
        let comment_text = if let Some(auto_idx) = comment_text.find(" ### ") {
            &comment_text[..auto_idx]
        } else {
            comment_text
        };
        (&rest[..idx], Some(comment_text.to_string()))
    } else {
        (rest, None)
    };

    // Split off time constraints (after @@@)
    let (main_part, time_constraints) = if let Some(idx) = main_part.find(" @@@ ") {
        let tc_str = main_part[idx + 5..].trim();
        let tcs: Vec<String> = tc_str.split_whitespace().map(|s| s.to_string()).collect();
        (&main_part[..idx], tcs)
    } else {
        (main_part, Vec::new())
    };

    // Split off relations (after >>>)
    let (main_part, relations) = if let Some(idx) = main_part.find(" >>> ") {
        let rel_str = main_part[idx + 5..].trim();
        let rels = parse_relations(rel_str);
        (&main_part[..idx], rels)
    } else {
        (main_part, Vec::new())
    };

    // Now parse: ID+PRIORITY DURATION "TITLE" {CHARTS} TAGS
    let mut chars = main_part.chars().peekable();

    // ID (until whitespace, !, ?, or ,)
    let mut id = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == '!' || c == '?' || c == ',' || c == '.' {
            break;
        }
        id.push(c);
        chars.next();
    }

    // Priority suffix
    let mut priority_chars = String::new();
    while let Some(&c) = chars.peek() {
        if c == '!' || c == '?' || c == ',' || c == '.' {
            priority_chars.push(c);
            chars.next();
        } else {
            break;
        }
    }
    let priority = match priority_chars.as_str() {
        "!!!" => "CRITICAL",
        "!!" => "HIGH",
        "!" => "MEDIUM",
        "..." => "LOW",
        ",,," => "LOWEST",
        "?" => "UNSURE",
        _ => "NEUTRAL",
    }.to_string();

    // Skip whitespace
    let remaining: String = chars.collect();
    let remaining = remaining.trim();

    // Find the quoted title
    let (duration, title, rest_after_title) = if let Some(q1) = remaining.find('"') {
        let before_quote = remaining[..q1].trim();
        let after_q1 = &remaining[q1 + 1..];
        let q2 = after_q1.find('"').unwrap_or(after_q1.len());
        let title = after_q1[..q2].to_string();
        let rest = if q2 + 1 < after_q1.len() { after_q1[q2 + 1..].trim() } else { "" };
        let duration = if before_quote.is_empty() { None } else { Some(before_quote.to_string()) };
        (duration, title, rest.to_string())
    } else {
        (None, remaining.to_string(), String::new())
    };

    // Parse charts: {..."..."}
    let (charts, rest_after_charts) = if let Some(brace_start) = rest_after_title.find('{') {
        if let Some(brace_end) = rest_after_title[brace_start..].find('}') {
            let charts_str = &rest_after_title[brace_start + 1..brace_start + brace_end];
            let charts: Vec<String> = charts_str
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let rest = rest_after_title[brace_start + brace_end + 1..].trim().to_string();
            (charts, rest)
        } else {
            (Vec::new(), rest_after_title)
        }
    } else {
        (Vec::new(), rest_after_title)
    };

    // Remaining is tags (comma-separated)
    let tags: Vec<String> = rest_after_charts
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if id.is_empty() {
        return None;
    }

    Some(ParsedGianttItem {
        id,
        title,
        status,
        priority,
        duration,
        charts,
        tags,
        relations,
        time_constraints,
        comment,
    })
}

/// Parse relation string like "⊢[dep1,dep2] ►[blocked1]"
fn parse_relations(s: &str) -> Vec<(String, Vec<String>)> {
    let mut result = Vec::new();
    let mut rest = s;

    let relation_symbols = [
        ("⊢", "requires"),
        ("⋲", "anyof"),
        ("≫", "supercharges"),
        ("∴", "indicates"),
        ("∪", "together"),
        ("⊟", "conflicts"),
        ("►", "blocks"),
        ("≻", "sufficient"),
    ];

    while !rest.is_empty() {
        let mut found = false;
        for (symbol, name) in &relation_symbols {
            if rest.starts_with(symbol) {
                rest = rest[symbol.len()..].trim_start();
                if rest.starts_with('[') {
                    if let Some(bracket_end) = rest.find(']') {
                        let targets: Vec<String> = rest[1..bracket_end]
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        result.push((name.to_string(), targets));
                        rest = rest[bracket_end + 1..].trim_start();
                    }
                }
                found = true;
                break;
            }
        }
        if !found {
            // Skip unknown character
            let mut chars = rest.chars();
            chars.next();
            rest = chars.as_str().trim_start();
        }
    }

    result
}

/// Import items from a .giantt file into the flow as CRDT ops.
/// Returns the number of items imported.
fn import_giantt_file(
    flow: &FullReplicaFlow<soradyne::convergent::giantt::GianttSchema>,
    path: &std::path::Path,
) -> Result<usize, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("read {}: {}", path.display(), e))?;

    // Check for #include directives
    let mut all_lines: Vec<String> = Vec::new();
    let parent_dir = path.parent().unwrap_or(std::path::Path::new("."));
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#include ") {
            let include_path = trimmed.trim_start_matches("#include ").trim();
            let include_full = parent_dir.join(include_path);
            if let Ok(included) = std::fs::read_to_string(&include_full) {
                for inc_line in included.lines() {
                    all_lines.push(inc_line.to_string());
                }
            } else {
                eprintln!("  Warning: could not read include file: {}", include_full.display());
            }
        } else {
            all_lines.push(line.to_string());
        }
    }

    let mut count = 0;
    for line in &all_lines {
        let Some(item) = parse_giantt_line(line) else { continue };

        // AddItem
        flow.apply_edit(Operation::add_item(&item.id, "GianttItem"))
            .map_err(|e| format!("AddItem {}: {}", item.id, e))?;

        // SetField: title (required)
        flow.apply_edit(Operation::set_field(&item.id, "title", Value::string(&item.title)))
            .map_err(|e| format!("SetField title: {}", e))?;

        // SetField: status
        if item.status != "NOT_STARTED" {
            flow.apply_edit(Operation::set_field(&item.id, "status", Value::string(&item.status)))
                .map_err(|e| format!("SetField status: {}", e))?;
        }

        // SetField: priority
        if item.priority != "NEUTRAL" {
            flow.apply_edit(Operation::set_field(&item.id, "priority", Value::string(&item.priority)))
                .map_err(|e| format!("SetField priority: {}", e))?;
        }

        // SetField: duration
        if let Some(ref dur) = item.duration {
            flow.apply_edit(Operation::set_field(&item.id, "duration", Value::string(dur)))
                .map_err(|e| format!("SetField duration: {}", e))?;
        }

        // SetField: comment
        if let Some(ref comment) = item.comment {
            flow.apply_edit(Operation::set_field(&item.id, "comment", Value::string(comment)))
                .map_err(|e| format!("SetField comment: {}", e))?;
        }

        // AddToSet: tags
        for tag in &item.tags {
            flow.apply_edit(Operation::add_to_set(&item.id, "tags", Value::string(tag)))
                .map_err(|e| format!("AddToSet tags: {}", e))?;
        }

        // AddToSet: charts
        for chart in &item.charts {
            flow.apply_edit(Operation::add_to_set(&item.id, "charts", Value::string(chart)))
                .map_err(|e| format!("AddToSet charts: {}", e))?;
        }

        // AddToSet: relations
        for (rel_type, targets) in &item.relations {
            let set_name = rel_type.as_str(); // "requires", "blocks", etc.
            for target in targets {
                flow.apply_edit(Operation::add_to_set(&item.id, set_name, Value::string(target)))
                    .map_err(|e| format!("AddToSet {}: {}", set_name, e))?;
            }
        }

        // AddToSet: timeConstraints
        for tc in &item.time_constraints {
            flow.apply_edit(Operation::add_to_set(&item.id, "timeConstraints", Value::string(tc)))
                .map_err(|e| format!("AddToSet timeConstraints: {}", e))?;
        }

        count += 1;
    }

    Ok(count)
}

// ---------------------------------------------------------------------------
// Direct TCP helpers (same as mdns_sync_peer)
// ---------------------------------------------------------------------------

async fn do_uuid_handshake(
    mut stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    our_id: Uuid,
) -> Result<(LanConnection, Uuid), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream.write_all(our_id.as_bytes()).await.map_err(|e| format!("write UUID: {e}"))?;
    let mut peer_bytes = [0u8; 16];
    stream.read_exact(&mut peer_bytes).await.map_err(|e| format!("read UUID: {e}"))?;
    let peer_id = Uuid::from_bytes(peer_bytes);
    let conn = LanConnection::from_stream(stream, BleAddress::Tcp(peer_addr));
    Ok((conn, peer_id))
}

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
    manager.add_direct_connection(peer_id, conn, TransportType::TcpDirect).await;
    println!("  Accepted direct TCP from {} ({})", &peer_id.to_string()[..8], peer_addr);
    Ok(())
}

async fn connect_to_peer(
    manager: &Arc<EnsembleManager>,
    device_id: Uuid,
    peer_id: Uuid,
    peer_addr: SocketAddr,
) -> Result<(), String> {
    let stream = tokio::net::TcpStream::connect(peer_addr)
        .await.map_err(|e| format!("TCP connect to {}: {}", peer_addr, e))?;
    let (conn, confirmed_peer_id) = do_uuid_handshake(stream, peer_addr, device_id).await?;
    if confirmed_peer_id != peer_id {
        eprintln!("  Warning: expected peer {} but got {} at {}", peer_id, confirmed_peer_id, peer_addr);
    }
    let conn: Arc<dyn BleConnection> = Arc::new(conn);
    manager.add_direct_connection(confirmed_peer_id, conn, TransportType::TcpDirect).await;
    println!("  Connected to {} via direct TCP", &confirmed_peer_id.to_string()[..8]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let identity = load_or_create_identity();
    let args: Vec<String> = std::env::args().collect();

    let mut listen_port: u16 = DEFAULT_PORT;
    let mut positional_args: Vec<String> = Vec::new();
    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate().skip(1) {
        if skip_next { skip_next = false; continue; }
        if arg == "--port" {
            listen_port = args.get(i + 1).expect("--port requires a value").parse().expect("--port must be a number");
            skip_next = true;
        } else if arg.starts_with("--port=") {
            listen_port = arg[7..].parse().expect("--port must be a number");
        } else {
            positional_args.push(arg.clone());
        }
    }

    match positional_args.first().map(|s| s.as_str()) {
        Some("init") => { cmd_init(&identity); return; }
        Some("join") => {
            cmd_join(&identity, positional_args.get(1).expect("Usage: join <CODE>"));
            return;
        }
        Some("accept") => {
            cmd_accept(&identity, positional_args.get(1).expect("Usage: accept <CODE>"));
            return;
        }
        Some(other) => {
            eprintln!("Unknown command: {other}");
            eprintln!("Usage: giantt_sync_peer [init | join <CODE> | accept <CODE>] [--port PORT]");
            return;
        }
        None => {}
    }

    let state = match load_state() {
        Some(s) => s,
        None => { eprintln!("No capsule configured. Run with `init` or `join` first."); return; }
    };
    if state.peers.is_empty() {
        eprintln!("No peers configured. Run `join` on the other device, then `accept` here.");
        return;
    }

    let identity = Arc::new(identity);
    let device_id = identity.device_id();
    let peer_addresses = load_peer_addresses();

    println!("rim giantt sync peer");
    println!("  Device:  {}", device_id);
    println!("  Capsule: {}", state.capsule_keys.capsule_id);
    println!("  Flow:    {}", state.flow_id);
    println!("  Listen:  0.0.0.0:{}", listen_port);
    println!("  Peers:   {}", state.peers.len());
    for (peer_id, _) in &state.peers {
        let addr_info = peer_addresses.get(peer_id).map(|a| format!(" @ {a}")).unwrap_or_default();
        println!("    - {}{}", peer_id, addr_info);
    }
    println!();

    // Build ensemble manager
    let mut all_piece_ids: Vec<Uuid> = vec![device_id];
    all_piece_ids.extend(state.peers.keys());

    let manager = EnsembleManager::new(
        Arc::clone(&identity), state.capsule_keys.clone(),
        all_piece_ids, state.peers.clone(), EnsembleConfig::default(),
    );

    // Create flow with GianttSchema
    let flow_storage = flow_storage_path();
    std::fs::create_dir_all(&flow_storage).ok();

    let flow_config = FlowConfig {
        id: state.flow_id,
        type_name: "drip_hosted:giantt".to_string(),
        params: serde_json::json!({}),
    };

    let mut flow = FullReplicaFlow::new_persistent(
        flow_config,
        soradyne::convergent::giantt::GianttSchema,
        DripHostPolicy::default(),
        device_id,
        identity.device_id_string(),
        flow_storage,
    );

    for (peer_uuid, _) in &state.peers {
        flow.register_party(&peer_uuid.to_string(), *peer_uuid);
    }

    use soradyne::flow::Flow;
    flow.set_ensemble(Arc::clone(manager.messenger()), Arc::clone(manager.topology()));
    flow.start().expect("Failed to start flow sync");

    // mDNS transport
    let mdns = MdnsTransport::new(false).expect("Failed to create mDNS transport");
    let central: Arc<dyn BleCentral> = Arc::new(mdns.create_device());
    let peripheral: Arc<dyn BlePeripheral> = Arc::new(mdns.create_device());
    println!("Starting mDNS discovery...");
    manager.start(central, peripheral).await;

    // TCP listener
    let tcp_listener = tokio::net::TcpListener::bind(
        SocketAddr::new("0.0.0.0".parse().unwrap(), listen_port),
    ).await.expect("Failed to bind TCP listener");
    println!("Direct TCP listening on port {}", tcp_listener.local_addr().unwrap().port());

    // Accept loop
    {
        let manager = Arc::clone(&manager);
        let known_peers: std::collections::HashSet<Uuid> = state.peers.keys().cloned().collect();
        tokio::spawn(async move {
            loop {
                let (stream, peer_addr) = match tcp_listener.accept().await { Ok(s) => s, Err(_) => break };
                let manager = Arc::clone(&manager);
                let known_peers = known_peers.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_direct_tcp(stream, peer_addr, &manager, &known_peers, device_id).await {
                        eprintln!("Direct TCP accept from {} failed: {}", peer_addr, e);
                    }
                });
            }
        });
    }

    // Outbound connections — everyone connects to everyone
    for (&peer_id, &peer_addr) in &peer_addresses {
        let manager = Arc::clone(&manager);
        tokio::spawn(async move {
            println!("  Connecting to {} at {} ...", &peer_id.to_string()[..8], peer_addr);
            if let Err(e) = connect_to_peer(&manager, device_id, peer_id, peer_addr).await {
                eprintln!("  {} not reachable ({}), they will connect to us", &peer_id.to_string()[..8], e);
            }
        });
    }

    // Reconnect on disconnect
    {
        let mut disconnect_rx = manager.messenger().disconnections();
        let manager = Arc::clone(&manager);
        let peer_addresses = peer_addresses.clone();
        tokio::spawn(async move {
            loop {
                let peer_id = match disconnect_rx.recv().await { Ok(id) => id, Err(_) => break };
                let Some(&peer_addr) = peer_addresses.get(&peer_id) else { continue };
                println!("  Peer {} disconnected — attempting reconnect", &peer_id.to_string()[..8]);
                let manager = Arc::clone(&manager);
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if let Err(e) = connect_to_peer(&manager, device_id, peer_id, peer_addr).await {
                        eprintln!("  Reconnect to {} failed ({}) — they will reconnect on restart", &peer_id.to_string()[..8], e);
                    }
                });
            }
        });
    }

    println!();
    println!("Commands:");
    println!("  import <path>     — import items from a .giantt file");
    println!("  list              — show all tasks");
    println!("  add <title>       — add a new task");
    println!("  status <id> <s>   — set status (todo/wip/done/blocked)");
    println!("  get <id>          — show all fields for a task");
    println!("  clear             — remove all items");
    println!("  peers             — show connected peers");
    println!("  quit              — exit");
    println!();

    // REPL
    let stdin = io::stdin();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        if line == "quit" || line == "exit" {
            break;
        } else if line == "list" {
            let doc = flow.document().read().unwrap();
            let state = doc.materialize();
            let mut items: Vec<_> = state.iter_existing().collect();
            items.sort_by_key(|(id, _)| id.to_string());
            if items.is_empty() {
                println!("  (no items)");
            } else {
                for (id, item) in items {
                    let title = item.fields.get("title")
                        .and_then(|v| match v { Value::String(s) => Some(s.as_str()), _ => None })
                        .unwrap_or("(untitled)");
                    let status = item.fields.get("status")
                        .and_then(|v| match v { Value::String(s) => Some(s.as_str()), _ => None })
                        .unwrap_or("TODO");
                    let priority = item.fields.get("priority")
                        .and_then(|v| match v { Value::String(s) => Some(s.as_str()), _ => None })
                        .unwrap_or("");
                    let pri_suffix = if priority.is_empty() { String::new() } else { format!(" [{}]", priority) };
                    println!("  {:12} {:12} {}{}", status, id, title, pri_suffix);
                }
            }
        } else if line == "peers" {
            match manager.topology().try_read() {
                Ok(topo) => {
                    let pieces: Vec<_> = topo.online_pieces.keys().collect();
                    if pieces.is_empty() { println!("  (no peers connected)"); }
                    else {
                        for id in pieces {
                            let label = if *id == device_id { " (self)" } else { "" };
                            println!("  {}{}", id, label);
                        }
                    }
                }
                Err(_) => println!("  (topology busy)"),
            }
        } else if line == "clear" {
            let doc = flow.document().read().unwrap();
            let ids: Vec<String> = doc.materialize().iter_existing().map(|(id, _)| id.to_string()).collect();
            drop(doc);
            for id in &ids {
                flow.apply_edit(Operation::remove_item(id)).expect("remove failed");
            }
            println!("  Removed {} items", ids.len());
        } else if let Some(path_str) = line.strip_prefix("import ") {
            let path = std::path::Path::new(path_str.trim());
            if !path.exists() {
                // Try expanding ~ to HOME
                let expanded = if path_str.trim().starts_with("~/") {
                    let home = std::env::var("HOME").unwrap_or_default();
                    PathBuf::from(home).join(&path_str.trim()[2..])
                } else {
                    path.to_path_buf()
                };
                if !expanded.exists() {
                    println!("  File not found: {}", expanded.display());
                    print!("> "); io::stdout().flush().ok();
                    continue;
                }
                match import_giantt_file(&flow, &expanded) {
                    Ok(n) => println!("  Imported {} items from {}", n, expanded.display()),
                    Err(e) => println!("  Import error: {}", e),
                }
            } else {
                match import_giantt_file(&flow, path) {
                    Ok(n) => println!("  Imported {} items from {}", n, path.display()),
                    Err(e) => println!("  Import error: {}", e),
                }
            }
        } else if let Some(title) = line.strip_prefix("add ") {
            let title = title.trim();
            if title.is_empty() { println!("  Usage: add <title>"); print!("> "); io::stdout().flush().ok(); continue; }
            let item_id = title.replace(' ', "_").to_lowercase();
            // Deduplicate if needed
            let item_id = {
                let doc = flow.document().read().unwrap();
                let state = doc.materialize();
                if state.get(&item_id).is_some() {
                    format!("{}_{}", item_id, &Uuid::new_v4().to_string()[..4])
                } else {
                    item_id
                }
            };
            flow.apply_edit(Operation::add_item(&item_id, "GianttItem")).expect("AddItem failed");
            flow.apply_edit(Operation::set_field(&item_id, "title", Value::string(title))).expect("SetField failed");
            println!("  Added: {} — {}", item_id, title);
        } else if let Some(rest) = line.strip_prefix("status ") {
            let parts: Vec<&str> = rest.trim().splitn(2, ' ').collect();
            if parts.len() != 2 {
                println!("  Usage: status <id> <todo|wip|done|blocked>");
                print!("> "); io::stdout().flush().ok(); continue;
            }
            let (id, new_status) = (parts[0], parts[1]);
            let status_value = match new_status.to_lowercase().as_str() {
                "todo" | "not_started" => "NOT_STARTED",
                "wip" | "in_progress" | "inprogress" => "IN_PROGRESS",
                "done" | "completed" => "COMPLETED",
                "blocked" => "BLOCKED",
                _ => { println!("  Unknown status: {}", new_status); print!("> "); io::stdout().flush().ok(); continue; }
            };
            match flow.apply_edit(Operation::set_field(id, "status", Value::string(status_value))) {
                Ok(_) => println!("  {} → {}", id, status_value),
                Err(e) => println!("  Error: {}", e),
            }
        } else if let Some(id) = line.strip_prefix("get ") {
            let id = id.trim();
            let doc = flow.document().read().unwrap();
            let state = doc.materialize();
            match state.get(&id.to_string()) {
                Some(item) => {
                    println!("  [{}]", id);
                    for (field, value) in &item.fields {
                        println!("    {}: {:?}", field, value);
                    }
                    for (set_name, elements) in &item.sets {
                        if !elements.is_empty() {
                            let elems: Vec<String> = elements.iter().map(|v| format!("{:?}", v)).collect();
                            println!("    {}: {}", set_name, elems.join(", "));
                        }
                    }
                }
                None => println!("  Item '{}' not found", id),
            }
        } else {
            println!("  Unknown command. Try: import, list, add, status, get, clear, peers, quit");
        }

        print!("> ");
        io::stdout().flush().ok();
    }

    println!("Shutting down...");
    manager.stop();
    flow.stop();
}
